/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use serde::Deserialize;

use crate::config::{lower, org_and_name};

use super::{trim, trim_react_native};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct PackageJson {
    name: String,
    repository: PackageJsonRepo,
    react_native: Option<String>,
    main: Option<String>,
    codegen_config: RnCodegenConfig,
}

impl PackageJson {
    pub(crate) fn raw_name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn name(&self) -> String {
        trim(&self.name)
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
        self.codegen_config
            .output_dir
            .android
            .clone()
            .unwrap_or("android/generated".to_string())
    }

    pub(crate) fn ios_codegen_output_dir(&self) -> String {
        self.codegen_config
            .output_dir
            .ios
            .clone()
            .unwrap_or("ios/generated".to_string())
    }

    pub(crate) fn repo(&self) -> &PackageJsonRepo {
        &self.repository
    }

    pub(crate) fn codegen(&self) -> &RnCodegenConfig {
        &self.codegen_config
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
    pub(crate) name: String,
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

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RnAndroidCodegenConfig {
    java_package_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RnOutputDirCodegenConfig {
    ios: Option<String>,
    android: Option<String>,
}
