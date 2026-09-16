/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};

use crate::switches::SwitchArgs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TsConfig {
    #[serde(default)]
    pub(crate) log_level: LogLevel,
    #[serde(default)]
    pub(crate) console_import: Option<String>,
    #[serde(default)]
    pub(crate) custom_types: HashMap<String, CustomTypeConfig>,
    #[serde(default)]
    pub(crate) strict_object_types: bool,
    /// When `true`, omit `// @ts-nocheck` from generated files so that
    /// `tsc` reports type errors. Defaults to `false` (generated files
    /// include `@ts-nocheck` to avoid noise in downstream projects).
    #[serde(default)]
    pub(crate) strict_type_checking: bool,
    /// When `true`, emit byte arrays (`Vec<u8>`) as `Uint8Array` instead of `ArrayBuffer`.
    #[serde(default)]
    pub(crate) strict_byte_arrays: bool,
    /// Give the named types and functions — or everything, when `true` — an
    /// `async`/`Promise<T>` surface. The FFI calls underneath stay
    /// synchronous: this is a migration aid toward moving them off the main
    /// thread.
    #[serde(default)]
    pub(crate) force_async: ForceAsync,
    /// Generate call bodies that `await` the player. For a player behind a
    /// message port. Implies `forceAsync = true`.
    #[serde(default)]
    pub(crate) async_delivery: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LogLevel {
    #[default]
    None,
    Debug,
    Verbose,
}

impl LogLevel {
    pub(crate) fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose)
    }
    pub(crate) fn is_debug(&self) -> bool {
        matches!(self, Self::Debug | Self::Verbose)
    }
}

impl TsConfig {
    pub(crate) fn is_verbose(&self) -> bool {
        self.log_level.is_verbose()
    }
    pub(crate) fn is_debug(&self) -> bool {
        self.log_level.is_debug()
    }

    /// Fold the command line into the config: `--async` sets `asyncDelivery`,
    /// and `asyncDelivery` needs a flavor that awaits its player at load, and
    /// forces every surface async.
    pub(crate) fn apply_switches(&mut self, switches: &SwitchArgs) -> anyhow::Result<()> {
        if switches.async_delivery {
            self.async_delivery = true;
        }
        if !self.async_delivery {
            return Ok(());
        }
        if !switches.flavor.supports_async_delivery() {
            anyhow::bail!(
                "asyncDelivery needs a flavor that awaits a player at load; `{}` does not",
                switches.flavor.as_str()
            );
        }
        match &self.force_async {
            ForceAsync::Named(_) => anyhow::bail!(
                "asyncDelivery makes every type async; remove the forceAsync list or set it to true"
            ),
            ForceAsync::All(_) => self.force_async = ForceAsync::All(true),
        }
        Ok(())
    }
}

/// `forceAsync` config value: a bool (all / nothing) or an explicit name list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ForceAsync {
    /// `forceAsync = true | false`
    All(bool),
    /// `forceAsync = ["ProcTraitMethods", "makeFlatTraitEnum"]`
    Named(Vec<String>),
}

impl Default for ForceAsync {
    fn default() -> Self {
        Self::All(false)
    }
}

impl ForceAsync {
    /// Whether the type or top-level function named `name` should render async.
    ///
    /// Both sides are normalized to UpperCamelCase before comparing, so a
    /// config entry of `traitRecord` also matches `TraitRecord` and
    /// `trait_record`.
    pub(crate) fn is_forced(&self, name: &str) -> bool {
        match self {
            ForceAsync::All(b) => *b,
            ForceAsync::Named(names) => {
                let target = name.to_upper_camel_case();
                names.iter().any(|n| n.to_upper_camel_case() == target)
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CustomTypeConfig {
    #[serde(default)]
    pub(crate) imports: Vec<(String, String)>,
    pub(crate) type_name: Option<String>,
    #[serde(alias = "lift")]
    pub(crate) into_custom: String,
    #[serde(alias = "lower")]
    pub(crate) from_custom: String,
}

impl CustomTypeConfig {
    pub(crate) fn lift(&self, variable: &str) -> String {
        self.into_custom.replace("{}", variable)
    }
    pub(crate) fn lower(&self, variable: &str) -> String {
        self.from_custom.replace("{}", variable)
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::switches::{AbiFlavor, SwitchArgs};

    #[test]
    fn all_true_forces_everything() {
        let fa = ForceAsync::All(true);
        assert!(fa.is_forced("AnyType"));
        assert!(fa.is_forced("any_function"));
    }

    #[test]
    fn default_is_all_false() {
        let fa = ForceAsync::default();
        assert!(!fa.is_forced("AnyType"));
        let cfg = TsConfig::default();
        assert!(!cfg.force_async.is_forced("AnyType"));
    }

    #[test]
    fn named_matches_across_case_conventions() {
        let fa = ForceAsync::Named(vec!["makeFlatTraitEnum".into(), "TraitRecord".into()]);
        // Candidates arrive in any spelling; all normalize to UpperCamelCase.
        assert!(fa.is_forced("make_flat_trait_enum")); // snake_case Rust fn
        assert!(fa.is_forced("makeFlatTraitEnum")); // lowerCamel TS name
        assert!(fa.is_forced("MakeFlatTraitEnum")); // UpperCamel
        assert!(fa.is_forced("TraitRecord"));
        assert!(fa.is_forced("trait_record"));
        assert!(!fa.is_forced("SomethingElse"));
    }

    #[test]
    fn deserializes_bool_and_list_forms() {
        let as_bool: TsConfig = toml::from_str("forceAsync = true").unwrap();
        assert!(as_bool.force_async.is_forced("Whatever"));

        let as_list: TsConfig =
            toml::from_str(r#"forceAsync = ["ProcTraitMethods", "makeFlatTraitEnum"]"#).unwrap();
        assert!(as_list.force_async.is_forced("ProcTraitMethods"));
        assert!(!as_list.force_async.is_forced("TraitRecord"));

        let default: TsConfig = toml::from_str("").unwrap();
        assert!(!default.force_async.is_forced("Whatever"));
    }

    fn switches(flavor: AbiFlavor, async_delivery: bool) -> SwitchArgs {
        SwitchArgs {
            flavor,
            async_delivery,
        }
    }

    #[test]
    fn async_delivery_deserializes() {
        let cfg: TsConfig = toml::from_str("asyncDelivery = true").unwrap();
        assert!(cfg.async_delivery);
        let cfg: TsConfig = toml::from_str("").unwrap();
        assert!(!cfg.async_delivery);
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn async_delivery_forces_everything_async() {
        let mut cfg: TsConfig = toml::from_str("asyncDelivery = true").unwrap();
        cfg.apply_switches(&switches(AbiFlavor::Wasm2, false))
            .unwrap();
        assert!(cfg.async_delivery);
        assert!(cfg.force_async.is_forced("Anything"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn cli_async_overrides_config() {
        let mut cfg: TsConfig = toml::from_str("").unwrap();
        cfg.apply_switches(&switches(AbiFlavor::Wasm2, true))
            .unwrap();
        assert!(cfg.async_delivery);
        assert!(cfg.force_async.is_forced("Anything"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn async_delivery_rejects_a_force_async_list() {
        let mut cfg: TsConfig =
            toml::from_str("asyncDelivery = true\nforceAsync = [\"Widget\"]").unwrap();
        let err = cfg
            .apply_switches(&switches(AbiFlavor::Wasm2, false))
            .unwrap_err()
            .to_string();
        assert!(err.contains("forceAsync"), "{err}");
    }

    #[test]
    fn async_delivery_rejects_a_flavor_without_a_player() {
        let mut cfg: TsConfig = toml::from_str("asyncDelivery = true").unwrap();
        let err = cfg
            .apply_switches(&switches(AbiFlavor::Jsi, false))
            .unwrap_err()
            .to_string();
        assert!(err.contains("jsi"), "{err}");
    }

    #[test]
    fn async_delivery_rejects_napi_despite_its_player() {
        // Napi's index initializes at module load, where nothing can await.
        let mut cfg: TsConfig = toml::from_str("asyncDelivery = true").unwrap();
        let err = cfg
            .apply_switches(&switches(AbiFlavor::Napi, false))
            .unwrap_err()
            .to_string();
        assert!(err.contains("napi"), "{err}");
    }

    #[test]
    fn cli_async_switch_is_rejected_on_napi_too() {
        let mut cfg: TsConfig = toml::from_str("").unwrap();
        let err = cfg
            .apply_switches(&switches(AbiFlavor::Napi, true))
            .unwrap_err()
            .to_string();
        assert!(err.contains("napi"), "{err}");
    }

    #[test]
    fn no_switch_leaves_config_alone() {
        let mut cfg: TsConfig = toml::from_str("forceAsync = [\"Widget\"]").unwrap();
        cfg.apply_switches(&switches(AbiFlavor::Jsi, false))
            .unwrap();
        assert!(!cfg.async_delivery);
        assert!(cfg.force_async.is_forced("Widget"));
        assert!(!cfg.force_async.is_forced("Other"));
    }
}
