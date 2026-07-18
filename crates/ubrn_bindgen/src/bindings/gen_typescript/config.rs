/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};

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
mod force_async_tests {
    use super::*;

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
}
