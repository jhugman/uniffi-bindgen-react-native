/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use serde::Deserialize;

use crate::config::{lower, org_and_name, ProjectConfig};

use super::{trim, trim_react_native};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct PackageJson {
    name: String,
    #[serde(default)]
    version: Option<String>,
    repository: PackageJsonRepo,
    #[serde(default)]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    browser: Option<String>,
    #[serde(alias = "react-native")]
    #[serde(default)]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    react_native: Option<String>,
    #[serde(default)]
    codegen_config: RnCodegenConfig,
}

impl PackageJson {
    pub(crate) fn raw_name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn name(&self) -> String {
        trim(&self.name)
    }

    pub(crate) fn version(&self) -> Option<String> {
        self.version.clone()
    }

    pub(crate) fn android_package_name(&self) -> String {
        self.codegen_config
            .android
            .java_package_name
            .clone()
            .unwrap_or_else(|| self.default_android_package_name())
    }

    fn default_android_package_name(&self) -> String {
        let (org, name) = org_and_name(&self.name);
        if let Some(org) = org {
            format!("com.{}.{}", lower(org), lower(name))
        } else {
            format!("com.{}", lower(&trim_react_native(name)))
        }
    }

    pub(crate) fn android_codegen_output_dir(&self) -> String {
        self.codegen_config.output_dir.android.clone()
    }

    pub(crate) fn ios_codegen_output_dir(&self) -> String {
        self.codegen_config.output_dir.ios.clone()
    }

    pub(crate) fn repo(&self) -> &PackageJsonRepo {
        &self.repository
    }

    pub(crate) fn codegen(&self) -> &RnCodegenConfig {
        &self.codegen_config
    }

    pub(crate) fn rn_entrypoint(&self) -> Option<String> {
        self.react_native.as_ref().cloned()
    }

    pub(crate) fn browser_entrypoint(&self) -> Option<String> {
        self.browser.as_ref().cloned()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PackageJsonRepo {
    pub(crate) url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct RnCodegenConfig {
    #[serde(default = "RnCodegenConfig::default_name")]
    pub(crate) name: String,
    #[serde(default = "RnCodegenConfig::default_js_src_dir")]
    pub(crate) js_srcs_dir: String,
    #[serde(default)]
    android: RnAndroidCodegenConfig,
    #[serde(default)]
    output_dir: RnOutputDirCodegenConfig,
}

impl Default for RnCodegenConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl RnCodegenConfig {
    fn default_js_src_dir() -> String {
        "src".to_string()
    }
    fn default_name() -> String {
        "RNNativeModuleSpec".to_string()
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RnAndroidCodegenConfig {
    java_package_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RnOutputDirCodegenConfig {
    #[serde(default = "default_ios_codegen_output_dir")]
    ios: String,
    #[serde(default = "default_android_codegen_output_dir")]
    android: String,
}

impl Default for RnOutputDirCodegenConfig {
    fn default() -> Self {
        Self {
            ios: default_ios_codegen_output_dir(),
            android: default_android_codegen_output_dir(),
        }
    }
}

fn default_android_codegen_output_dir() -> String {
    "android/generated".to_string()
}

fn default_ios_codegen_output_dir() -> String {
    "ios/generated".to_string()
}
