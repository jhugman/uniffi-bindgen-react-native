/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use serde::Deserialize;
use std::{fmt::Display, fs, process::Command, str::FromStr};

use clap::Args;

use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use ubrn_common::{mk_dir, rm_dir, run_cmd, CrateMetadata};

use crate::{
    building::{CommonBuildArgs, ExtraArgs},
    config::ProjectConfig,
    workspace,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AndroidConfig {
    #[serde(default = "AndroidConfig::default_directory")]
    pub(crate) directory: String,

    #[serde(default = "AndroidConfig::default_jni_libs")]
    pub(crate) jni_libs: String,

    #[serde(default = "AndroidConfig::default_targets")]
    pub(crate) targets: Vec<String>,

    #[serde(default = "AndroidConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(default = "AndroidConfig::default_platform", alias = "platform")]
    pub(crate) api_level: usize,

    #[allow(dead_code)]
    #[serde(default = "AndroidConfig::default_package_name")]
    pub(crate) package_name: String,
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

    fn default_targets() -> Vec<String> {
        let args: &[&str] = &[
            "aarch64-linux-android",
            "armv7-linux-androideabi",
            "i686-linux-android",
            "x86_64-linux-android",
        ];
        args.iter().map(|s| s.to_string()).collect()
    }

    fn default_platform() -> usize {
        21
    }

    fn default_directory() -> String {
        "android".to_string()
    }

    fn default_jni_libs() -> String {
        "src/main/jniLibs".to_string()
    }
}

impl AndroidConfig {
    pub(crate) fn directory(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        project_root.join(&self.directory)
    }

    pub(crate) fn jni_libs(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join(&self.jni_libs)
    }

    fn java_src(&self) -> String {
        "src/main/java".to_string()
    }

    pub(crate) fn src_main_java_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.directory(project_root).join(self.java_src())
    }

    pub(crate) fn codegen_package_dir(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.src_main_java_dir(project_root)
            .join(self.package_name.replace('.', "/"))
    }
}

#[derive(Args, Debug)]
pub(crate) struct AndroidArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl AndroidArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config: ProjectConfig = self.project_config()?;
        let project_root = config.project_root();
        let crate_ = &config.crate_;
        let rust_dir = crate_.directory()?;
        let manifest_path = crate_.manifest_path()?;

        let android = &config.android;

        let jni_libs = android.jni_libs(project_root);
        rm_dir(&jni_libs)?;

        let mut target_files: Vec<_> = Vec::new();
        for target in &android.targets {
            let target = target.parse::<Target>()?;
            let mut cmd = Command::new("cargo");
            cmd.arg("ndk")
                .arg("--manifest-path")
                .arg(&manifest_path)
                .arg("--target")
                .arg(target.to_string())
                .arg("--platform")
                .arg(format!("{}", android.api_level));

            if !self.common_args.release {
                cmd.arg("--no-strip");
            }

            cmd.arg("--").arg("build");
            if self.common_args.release {
                cmd.arg("--release");
            }

            cmd.args(android.cargo_extras.clone());

            run_cmd(cmd.current_dir(&rust_dir))?;
            let metadata = crate_.metadata()?;
            let src_lib = metadata.library_path(
                Some(target.triple()),
                CrateMetadata::profile(self.common_args.release),
            )?;
            let dst_dir = jni_libs.join(target.to_string());
            mk_dir(&dst_dir)?;

            let dst_lib = dst_dir.join(metadata.library_file(Some(target.triple())));
            fs::copy(&src_lib, &dst_lib)?;

            target_files.push(src_lib);
        }

        Ok(target_files)
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        self.config.clone().try_into()
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
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
