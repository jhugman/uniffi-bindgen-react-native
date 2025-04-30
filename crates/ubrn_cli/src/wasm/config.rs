/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{fmt::Display, str::FromStr};

use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{
    config::{rust_crate::CrateConfig, ExtraArgs},
    workspace,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmConfig {
    #[serde(default = "WasmConfig::default_manifest_path")]
    #[serde(deserialize_with = "CrateConfig::validate_manifest_path")]
    pub(crate) manifest_path: String,

    #[serde(default = "WasmConfig::default_wasm_crate_name")]
    pub(crate) wasm_crate_name: String,

    #[serde(default)]
    pub(crate) features: Option<Vec<String>>,

    /// Has this crate been added to a workspace. Default is false.
    #[serde(default = "WasmConfig::default_is_workspace")]
    pub(crate) workspace: bool,

    #[allow(unused)]
    #[serde(default = "WasmConfig::default_targets")]
    pub(crate) targets: Vec<Target>,

    #[allow(unused)]
    #[serde(default = "WasmConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,
}

impl Default for WasmConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl WasmConfig {
    fn default_wasm_crate_name() -> String {
        workspace::package_json().name()
    }
    fn default_manifest_path() -> String {
        "wasm/generated/Cargo.toml".to_string()
    }
    fn default_targets() -> Vec<Target> {
        vec![Target::Wasm32UnknownUnknown]
    }
    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }
    fn default_is_workspace() -> bool {
        false
    }
}

impl WasmConfig {
    pub(crate) fn wasm_crate_name(&self) -> String {
        self.wasm_crate_name.clone()
    }
    pub(crate) fn manifest_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.manifest_path)
    }
    pub(crate) fn crate_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.manifest_path(project_root)
            .parent()
            .unwrap_or(&Utf8PathBuf::new())
            .into()
    }
}

#[allow(unused)]
#[derive(Debug, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
pub enum Target {
    #[default]
    #[serde(rename = "wasm32-unknown-unknown")]
    Wasm32UnknownUnknown,
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "wasm32-unknown-unknown" => Self::Wasm32UnknownUnknown,
            _ => return Err(anyhow::anyhow!("Unsupported target: '{s}'")),
        })
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
        })
    }
}

#[allow(unused)]
impl Target {
    pub fn triple(&self) -> &'static str {
        match self {
            Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
        }
    }
}
