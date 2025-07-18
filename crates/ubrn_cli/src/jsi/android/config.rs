/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{fmt::Display, str::FromStr};

use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{config::ExtraArgs, workspace};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AndroidConfig {
    #[serde(default = "AndroidConfig::default_directory")]
    pub(crate) directory: String,

    #[serde(default = "AndroidConfig::default_jni_libs")]
    pub(crate) jni_libs: String,

    #[serde(default = "AndroidConfig::default_targets")]
    pub(crate) targets: Vec<Target>,

    #[serde(default = "AndroidConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(default = "AndroidConfig::default_platform", alias = "platform")]
    pub(crate) api_level: usize,

    #[serde(default = "AndroidConfig::default_package_name")]
    pub(crate) package_name: String,

    #[serde(default = "AndroidConfig::default_codegen_output_dir")]
    pub(crate) codegen_output_dir: String,
    
    #[serde(default = "AndroidConfig::default_use_shared_library")]
    pub(crate) use_shared_library: bool,
}

impl Default for AndroidConfig {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl AndroidConfig {
    fn default_package_name() -> String {
        workspace::package_json().android_package_name()
    }

    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }

    fn default_targets() -> Vec<Target> {
        vec![
            Target::Arm64V8a,
            Target::ArmeabiV7a,
            Target::X86,
            Target::X86_64,
        ]
    }

    fn default_platform() -> usize {
        // This is minSdkVersion supported for 0.75.4
        // For 0.76, this increases to 24.
        // While we still support 0.75.4, we should not raise this.
        // If users want to raise this, they can change the platform in the
        // ubrn.config.yaml.
        23
    }

    fn default_directory() -> String {
        "android".to_string()
    }

    fn default_jni_libs() -> String {
        "src/main/jniLibs".to_string()
    }

    fn default_codegen_output_dir() -> String {
        workspace::package_json().android_codegen_output_dir()
    }

    fn default_use_shared_library() -> bool {
        false
    }
}

impl AndroidConfig {
    pub(crate) fn directory(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.directory)
    }

    pub(crate) fn codegen_output_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.codegen_output_dir)
    }

    pub(crate) fn jni_libs(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join(&self.jni_libs)
    }

    fn main_src(&self) -> String {
        "src/main".to_string()
    }

    fn java_src(&self) -> String {
        "src/main/java".to_string()
    }

    pub(crate) fn package_name(&self) -> String {
        self.package_name.clone()
    }

    pub(crate) fn src_main_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join(self.main_src())
    }

    pub(crate) fn src_main_java_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join(self.java_src())
    }

    pub(crate) fn codegen_package_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.src_main_java_dir(project_root)
            .join(self.package_name.replace('.', "/"))
    }
}

#[derive(Debug, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
pub enum Target {
    #[serde(rename = "armeabi-v7a")]
    ArmeabiV7a,
    #[default]
    #[serde(rename = "arm64-v8a")]
    Arm64V8a,
    #[serde(rename = "x86")]
    X86,
    #[serde(rename = "x86_64")]
    X86_64,
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            // match android style architectures
            "armeabi-v7a" => Target::ArmeabiV7a,
            "arm64-v8a" => Target::Arm64V8a,
            "x86" => Target::X86,
            "x86_64" => Target::X86_64,
            // match rust triple architectures
            "armv7-linux-androideabi" => Target::ArmeabiV7a,
            "aarch64-linux-android" => Target::Arm64V8a,
            "i686-linux-android" => Target::X86,
            "x86_64-linux-android" => Target::X86_64,
            _ => return Err(anyhow::anyhow!("Unsupported target: '{s}'")),
        })
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Target::ArmeabiV7a => "armeabi-v7a",
            Target::Arm64V8a => "arm64-v8a",
            Target::X86 => "x86",
            Target::X86_64 => "x86_64",
        })
    }
}

impl Target {
    pub fn triple(&self) -> &'static str {
        match self {
            Target::ArmeabiV7a => "armv7-linux-androideabi",
            Target::Arm64V8a => "aarch64-linux-android",
            Target::X86 => "i686-linux-android",
            Target::X86_64 => "x86_64-linux-android",
        }
    }
}
