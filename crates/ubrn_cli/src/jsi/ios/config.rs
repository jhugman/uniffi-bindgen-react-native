/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{env::consts::ARCH, fmt::Display, str::FromStr};

use anyhow::{Context, Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};

use crate::{
    config::{org_and_name, trim_react_native, ExtraArgs},
    workspace,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IOsConfig {
    #[serde(default = "IOsConfig::default_directory")]
    pub(crate) directory: String,

    #[serde(default = "IOsConfig::default_framework_name")]
    pub(crate) framework_name: String,

    #[serde(default = "IOsConfig::default_xcodebuild_extras")]
    pub(crate) xcodebuild_extras: ExtraArgs,

    #[serde(default = "IOsConfig::default_targets")]
    pub(crate) targets: Vec<Target>,

    #[serde(default = "IOsConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(default = "IOsConfig::default_codegen_output_dir")]
    pub(crate) codegen_output_dir: String,
}

impl IOsConfig {
    fn default_directory() -> String {
        "ios".to_string()
    }

    fn default_framework_name() -> String {
        let name = workspace::package_json().name();
        let (org, name) = org_and_name(&name);
        let prefix = if let Some(org) = org {
            format!("{org}_{name}").to_upper_camel_case()
        } else {
            trim_react_native(name).to_upper_camel_case()
        };
        format!("{prefix}Framework")
    }

    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }

    fn default_xcodebuild_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }

    fn default_targets() -> Vec<Target> {
        let sim_target = if ARCH.starts_with("x86") {
            "x86_64-apple-ios"
        } else {
            "aarch64-apple-ios-sim"
        };
        let args: &[&str] = &["aarch64-apple-ios", sim_target];
        args.iter().map(|s| Target::from_str(s).unwrap()).collect()
    }

    fn default_codegen_output_dir() -> String {
        workspace::package_json().ios_codegen_output_dir()
    }
}

impl Default for IOsConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl IOsConfig {
    pub(crate) fn directory(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.directory)
    }

    pub(crate) fn codegen_output_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.codegen_output_dir)
    }

    pub(crate) fn framework_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = format!("{}.xcframework", self.framework_name);
        project_root.join(filename)
    }

    pub(crate) fn native_bindings_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join("swift")
    }
}

/// A specific build target supported by the SDK.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct Target {
    pub(crate) triple: String,
    pub(crate) platform: Platform,
    pub(crate) description: String,
}

/// The platform for which a particular target can run on.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub(crate) enum Platform {
    Macos,
    Ios,
    IosSimulator,
}

impl Platform {
    /// The name of the subfolder in which to place the library for the platform
    /// once all architectures are lipo'd together.
    pub(crate) fn lib_folder_name(&self) -> &str {
        match self {
            Platform::Macos => "macos",
            Platform::Ios => "ios",
            Platform::IosSimulator => "ios-simulator",
        }
    }
}

/// The list of targets supported by the SDK.
pub(crate) fn supported_targets() -> Vec<Target> {
    vec![
        Target {
            triple: "aarch64-apple-ios".into(),
            platform: Platform::Ios,
            description: "iOS".into(),
        },
        Target {
            triple: "aarch64-apple-darwin".into(),
            platform: Platform::Macos,
            description: "macOS (Apple Silicon)".into(),
        },
        Target {
            triple: "x86_64-apple-darwin".into(),
            platform: Platform::Macos,
            description: "macOS (Intel)".into(),
        },
        Target {
            triple: "aarch64-apple-ios-sim".into(),
            platform: Platform::IosSimulator,
            description: "iOS Simulator (Apple Silicon)".into(),
        },
        Target {
            triple: "x86_64-apple-ios".into(),
            platform: Platform::IosSimulator,
            description: "iOS Simulator (Intel)".into(),
        },
    ]
}

/// Additional work to make Target serializable/deserializable
/// to and from a string without another dependency
impl FromStr for Target {
    type Err = Error;

    fn from_str(t: &str) -> Result<Self, Self::Err> {
        supported_targets()
            .iter()
            .find(|target| target.triple == t)
            .cloned()
            .with_context(|| format!("Unsupported target: '{t}'"))
    }
}

impl TryFrom<String> for Target {
    type Error = Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Target::from_str(&value)
    }
}

impl From<Target> for String {
    fn from(value: Target) -> Self {
        value.to_string()
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.triple)
    }
}
