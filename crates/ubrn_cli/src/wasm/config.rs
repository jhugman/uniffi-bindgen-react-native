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
    config::{rust_crate::CrateConfig, ExtraArgs, ProjectConfig},
    workspace,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmConfig {
    #[serde(default = "WasmConfig::default_manifest_path")]
    #[serde(deserialize_with = "CrateConfig::validate_manifest_path")]
    pub(crate) manifest_path: String,

    #[serde(default)]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    pub(crate) manifest_patch_file: Option<String>,

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

    #[serde(default)]
    pub(crate) target: WasmTarget,

    #[serde(default = "WasmConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(default = "WasmConfig::default_wasm_bindgen_extras")]
    pub(crate) wasm_bindgen_extras: ExtraArgs,

    #[serde(default = "WasmConfig::default_runtime_version")]
    pub(crate) runtime_version: String,

    #[serde(alias = "ts", alias = "typescript")]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    #[serde(default)]
    pub(crate) ts_bindings: Option<String>,

    #[serde(default = "WasmConfig::default_entrypoint")]
    #[serde(deserialize_with = "ProjectConfig::relative_path")]
    pub(crate) entrypoint: String,
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
        "rust_modules/wasm/Cargo.toml".to_string()
    }
    fn default_targets() -> Vec<Target> {
        vec![Target::Wasm32UnknownUnknown]
    }
    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }
    fn default_wasm_bindgen_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }
    fn default_is_workspace() -> bool {
        false
    }
    fn default_runtime_version() -> String {
        format!("={}", env!("CARGO_PKG_VERSION"))
    }
    fn default_entrypoint() -> String {
        let package_json = workspace::package_json();
        package_json
            .browser_entrypoint()
            .unwrap_or_else(|| "src/index.web.ts".to_string())
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
    pub(crate) fn runtime_version(&self) -> String {
        self.runtime_version.clone()
    }
    pub(crate) fn entrypoint(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.entrypoint)
    }
}

#[derive(Debug, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
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

impl Target {
    pub fn triple(&self) -> &'static str {
        match self {
            Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WasmTarget {
    Bundler,
    Nodejs,
    #[default]
    Web,
    NoModules,
    Deno,
    ExperimentalNodejsModule,
}

impl FromStr for WasmTarget {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bundler" => Self::Bundler,
            "nodejs" => Self::Nodejs,
            "web" => Self::Web,
            "no-modules" => Self::NoModules,
            "deno" => Self::Deno,
            "experimental-nodejs-module" => Self::ExperimentalNodejsModule,
            _ => return Err(anyhow::anyhow!("Unsupported target: '{s}'")),
        })
    }
}

impl Display for WasmTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bundler => "bundler",
            Self::Nodejs => "nodejs",
            Self::Web => "web",
            Self::NoModules => "no-modules",
            Self::Deno => "deno",
            Self::ExperimentalNodejsModule => "experimental-nodejs-module",
        })
    }
}
