/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::workspace;

// Define our own trim function since it's private in config module
fn trim(name: &str) -> String {
    name.trim_matches('-').trim_matches('_').to_string()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurboModulesConfig {
    #[serde(default = "TurboModulesConfig::default_cpp_dir")]
    pub(crate) cpp: String,
    #[serde(default = "TurboModulesConfig::default_ts_dir")]
    pub(crate) ts: String,
    #[serde(default = "TurboModulesConfig::default_spec_name", alias = "spec")]
    pub(crate) spec_name: String,

    #[serde(default = "TurboModulesConfig::default_name")]
    pub(crate) name: String,
}

impl TurboModulesConfig {
    fn default_name() -> String {
        let package_json = workspace::package_json();
        package_json.codegen().name.clone()
    }

    fn default_cpp_dir() -> String {
        "cpp".to_string()
    }

    fn default_ts_dir() -> String {
        let package_json = workspace::package_json();
        package_json.codegen().js_srcs_dir.clone()
    }

    fn default_spec_name() -> String {
        let package_json = workspace::package_json();
        let codegen_name = &package_json.codegen().name;
        trim(codegen_name)
    }
}

impl Default for TurboModulesConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl TurboModulesConfig {
    pub(crate) fn cpp_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.cpp)
    }

    pub(crate) fn ts_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.ts)
    }

    #[allow(dead_code)]
    pub(crate) fn spec_name(&self) -> String {
        self.spec_name.clone()
    }

    pub(crate) fn name(&self) -> String {
        self.name.clone()
    }
}
