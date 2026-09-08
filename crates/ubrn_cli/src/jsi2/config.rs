/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use serde::Deserialize;

use crate::{config::ProjectConfig, jsi::android::config::Target as AndroidTarget, workspace};

/// The `jsi2:` section of `ubrn.config.yaml`. The player loads the crate as a
/// shared library, so this is only how to build that library and where the
/// TypeScript goes; `android:` and `ios:` still supply directories, API level
/// and cargo extras.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Jsi2Config {
    /// Where the TypeScript bindings go; falls back to `bindings.ts`.
    #[serde(alias = "ts", alias = "typescript")]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    #[serde(default)]
    pub(crate) ts_bindings: Option<String>,

    /// The ABIs the library ships. 64-bit only: Google Play has required it
    /// since 2019, and the player ships the same two.
    #[serde(default = "Jsi2Config::default_android_targets")]
    pub(crate) android_targets: Vec<AndroidTarget>,

    /// Stamped into the dylib (IPHONEOS_DEPLOYMENT_TARGET) and the framework's
    /// Info.plist. React Native 0.77, the compat floor, requires 15.1.
    // Read by the iOS build, which does not exist yet.
    #[allow(dead_code)]
    #[serde(default = "Jsi2Config::default_min_ios_version")]
    pub(crate) min_ios_version: String,

    /// Reverse-DNS prefix of the framework's CFBundleIdentifier. Defaults to
    /// the Android package name, the one reverse-DNS name every RN library has.
    // Read by the iOS build, which does not exist yet.
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) bundle_id_prefix: Option<String>,
}

impl Default for Jsi2Config {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl Jsi2Config {
    fn default_android_targets() -> Vec<AndroidTarget> {
        vec![AndroidTarget::Arm64V8a, AndroidTarget::X86_64]
    }

    fn default_min_ios_version() -> String {
        "15.1".to_string()
    }

    // Read by the iOS build, which does not exist yet.
    #[allow(dead_code)]
    pub(crate) fn bundle_id_prefix(&self) -> String {
        self.bundle_id_prefix
            .clone()
            .unwrap_or_else(|| workspace::package_json().android_package_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_shipped_abis_and_the_rn_floor() {
        let config: Jsi2Config = toml::from_str("").expect("empty section parses");
        assert_eq!(
            config.android_targets,
            vec![AndroidTarget::Arm64V8a, AndroidTarget::X86_64]
        );
        assert_eq!(config.min_ios_version, "15.1");
        assert!(config.ts_bindings.is_none());
        assert!(config.bundle_id_prefix.is_none());
    }

    #[test]
    fn every_field_is_configurable() {
        let config: Jsi2Config = toml::from_str(
            r#"
ts = "./src/gen"
androidTargets = ["arm64-v8a", "armeabi-v7a"]
minIosVersion = "16.0"
bundleIdPrefix = "dev.example"
"#,
        )
        .expect("parses");
        assert_eq!(config.ts_bindings.as_deref(), Some("src/gen"));
        assert_eq!(
            config.android_targets,
            vec![AndroidTarget::Arm64V8a, AndroidTarget::ArmeabiV7a]
        );
        assert_eq!(config.min_ios_version, "16.0");
        assert_eq!(config.bundle_id_prefix(), "dev.example");
    }
}
