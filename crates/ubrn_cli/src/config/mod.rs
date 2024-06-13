/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod npm;

use camino::{Utf8Path, Utf8PathBuf};
pub(crate) use npm::PackageJson;

use serde::Deserialize;

use crate::{android::AndroidConfig, ios::IOsConfig, rust::CrateConfig};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfig {
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
    pub(crate) fn project_root(&self) -> &Utf8Path {
        &self.crate_.project_root
    }
}
