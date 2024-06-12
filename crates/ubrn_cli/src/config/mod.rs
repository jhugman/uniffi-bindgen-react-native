/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod npm;

use camino::Utf8Path;
pub(crate) use npm::PackageJson;

use serde::Deserialize;

use crate::{android::AndroidConfig, ios::IOsConfig, rust::CrateConfig, workspace};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfig {
    #[serde(default = "ProjectConfig::default_name")]
    pub(crate) name: String,

    #[serde(rename = "crate")]
    pub(crate) crate_: CrateConfig,

    #[serde(default)]
    pub(crate) android: AndroidConfig,

    #[serde(default)]
    pub(crate) ios: IOsConfig,

    #[serde(default)]
    pub(crate) bindings: BindingsConfig,

    #[serde(default, rename = "turboModule")]
    pub(crate) tm: TurboModulesConfig,
}

impl ProjectConfig {
    fn default_name() -> String {
        workspace::package_json().name()
    }
}

impl ProjectConfig {
    pub(crate) fn project_root(&self) -> &Utf8Path {
        &self.crate_.project_root
    }
}

impl ProjectConfig {
    pub(crate) fn name(&self) -> String {
        self.name.clone()
    }
}
