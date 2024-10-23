/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod npm;

use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;
use heck::ToUpperCamelCase;
pub(crate) use npm::PackageJson;

use serde::Deserialize;

use crate::{android::AndroidConfig, ios::IOsConfig, rust::CrateConfig, workspace};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectConfig {
    #[serde(default = "ProjectConfig::default_name")]
    pub(crate) name: String,

    #[serde(default = "ProjectConfig::default_repository")]
    pub(crate) repository: String,

    #[serde(rename = "rust", alias = "crate")]
    pub(crate) crate_: CrateConfig,

    #[serde(default)]
    pub(crate) android: AndroidConfig,

    #[serde(default)]
    pub(crate) ios: IOsConfig,

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
        self.cpp_filename()
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
