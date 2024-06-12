/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use heck::ToUpperCamelCase;
use serde::Deserialize;

use super::trim_react_native;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct PackageJson {
    name: String,
    react_native: Option<String>,
    main: Option<String>,
    codegen_config: RnCodegenConfig,
}

impl PackageJson {
    pub(crate) fn raw_name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn name(&self) -> String {
        trim_react_native(&self.name)
    }

    pub(crate) fn android_package_name(&self) -> String {
        self.codegen_config
            .android
            .java_package_name
            .clone()
            .unwrap_or_else(|| format!("com.{}", self.name().to_upper_camel_case().to_lowercase()))
    }

    pub(crate) fn codegen(&self) -> &RnCodegenConfig {
        &self.codegen_config
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct RnCodegenConfig {
    pub(crate) name: String,
    pub(crate) js_srcs_dir: String,
    #[serde(default)]
    android: RnAndroidCodegenConfig,
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
