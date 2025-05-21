/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod npm;
pub(crate) mod rust_crate;

use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;
use heck::ToUpperCamelCase;
use serde::Deserialize;

pub(crate) use npm::PackageJson;

use crate::{
    config::rust_crate::CrateConfig,
    jsi::{android::AndroidConfig, crossplatform::TurboModulesConfig, ios::IOsConfig},
    workspace,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfig {
    #[serde(default = "ProjectConfig::default_name")]
    pub(crate) name: String,

    #[serde(default = "ProjectConfig::default_version", alias = "version")]
    pub(crate) project_version: String,

    #[serde(default = "ProjectConfig::default_repository")]
    pub(crate) repository: String,

    #[serde(rename = "rust", alias = "crate")]
    pub(crate) crate_: CrateConfig,

    #[serde(default)]
    pub(crate) android: AndroidConfig,

    #[serde(default)]
    pub(crate) ios: IOsConfig,

    #[cfg(feature = "wasm")]
    #[serde(default, alias = "web")]
    pub(crate) wasm: crate::wasm::WasmConfig,

    #[serde(default)]
    pub(crate) bindings: BindingsConfig,

    #[serde(default, rename = "turboModule")]
    pub(crate) tm: TurboModulesConfig,

    /// Set of globs of file paths not to be overwritten by
    /// the `generate` commands.
    #[serde(default, rename = "noOverwrite")]
    pub(crate) exclude_files: GlobSet,
}

impl ProjectConfig {
    fn default_name() -> String {
        workspace::package_json().raw_name()
    }

    fn default_repository() -> String {
        let package_json = workspace::package_json();
        let url = &package_json.repo().url;
        url.trim_start_matches("git+").to_string()
    }
    fn default_version() -> String {
        let package_json = workspace::package_json();
        package_json
            .version()
            .unwrap_or_else(|| "0.1.0".to_string())
    }
}

fn trim(name: &str) -> String {
    name.trim_matches('-').trim_matches('_').to_string()
}

#[allow(dead_code)]
fn trim_rn(name: &str) -> String {
    trim_react_native(strip_prefix(name, "RN"))
}

fn strip_prefix<'a>(name: &'a str, prefix: &str) -> &'a str {
    name.strip_prefix(prefix).unwrap_or(name)
}

pub(crate) fn trim_react_native(name: &str) -> String {
    strip_prefix(strip_prefix(name, "ReactNative"), "react-native")
        .trim_matches('-')
        .trim_matches('_')
        .to_string()
}

pub(crate) fn org_and_name(name: &str) -> (Option<&str>, &str) {
    if let Some((left, right)) = name.split_once('/') {
        let org = left.strip_prefix('@').unwrap_or(left);
        (Some(org), right)
    } else {
        (None, name)
    }
}

pub(crate) fn lower(s: &str) -> String {
    s.to_upper_camel_case().to_lowercase()
}

impl ProjectConfig {
    pub(crate) fn project_root(&self) -> &Utf8Path {
        &self.crate_.project_root
    }

    pub(crate) fn module_cpp(&self) -> String {
        let (org, name) = org_and_name(&self.name);
        if org.is_some() {
            name.to_upper_camel_case()
        } else {
            trim_react_native(name).to_upper_camel_case()
        }
    }

    pub(crate) fn ubrn_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[allow(dead_code)]
    pub(crate) fn project_version(&self) -> String {
        self.project_version.clone()
    }
}

impl ProjectConfig {
    pub(crate) fn raw_name(&self) -> &str {
        &self.name
    }

    pub(crate) fn repository(&self) -> &str {
        &self.repository
    }

    pub(crate) fn cpp_namespace(&self) -> String {
        let (org, name) = org_and_name(&self.name);
        if let Some(org) = org {
            format!("{}_{}", lower(org), lower(name))
        } else {
            lower(&trim_react_native(name))
        }
    }

    pub(crate) fn cpp_filename(&self) -> String {
        use heck::ToKebabCase;
        self.raw_name().to_kebab_case()
    }

    pub(crate) fn podspec_filename(&self) -> String {
        self.module_cpp().to_upper_camel_case()
    }

    pub(crate) fn codegen_filename(&self) -> String {
        format!("Native{}", self.spec_name())
    }

    pub(crate) fn spec_name(&self) -> String {
        self.module_cpp()
    }

    pub(crate) fn exclude_files(&self) -> &GlobSet {
        &self.exclude_files
    }

    pub(crate) fn wasm_bindings_ts_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.wasm
            .ts_bindings
            .as_deref()
            .map(|ts| project_root.join(ts))
            .unwrap_or_else(|| self.bindings.ts_path(project_root))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BindingsConfig {
    #[serde(default = "BindingsConfig::default_cpp_dir")]
    pub(crate) cpp: String,
    #[serde(default = "BindingsConfig::default_ts_dir")]
    pub(crate) ts: String,

    #[serde(default)]
    pub(crate) uniffi_toml: Option<String>,
}

impl BindingsConfig {
    fn default_cpp_dir() -> String {
        "cpp/generated".to_string()
    }

    fn default_ts_dir() -> String {
        "src/generated".to_string()
    }
}

impl Default for BindingsConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl BindingsConfig {
    pub(crate) fn cpp_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.cpp)
    }

    pub(crate) fn ts_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.ts)
    }

    pub(crate) fn uniffi_toml_path(&self, project_root: &Utf8Path) -> Option<Utf8PathBuf> {
        self.uniffi_toml.as_ref().map(|f| project_root.join(f))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ExtraArgs {
    AsList(Vec<String>),
    AsString(String),
}

impl IntoIterator for ExtraArgs {
    type Item = String;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ExtraArgs::AsList(v) => v.into_iter(),
            ExtraArgs::AsString(s) => s
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .into_iter(),
        }
    }
}

impl Default for ExtraArgs {
    fn default() -> Self {
        Self::AsList(Default::default())
    }
}

impl From<&[&str]> for ExtraArgs {
    fn from(value: &[&str]) -> Self {
        let vec = value.iter().map(|&s| s.to_string()).collect();
        ExtraArgs::AsList(vec)
    }
}
