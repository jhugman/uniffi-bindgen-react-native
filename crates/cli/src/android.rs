/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use serde::Deserialize;
use std::process::Command;

use clap::Args;

use anyhow::Result;
use camino::Utf8PathBuf;
use uniffi_common::{rm_dir, run_cmd};

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

    #[serde(default = "AndroidConfig::default_platform")]
    pub(crate) api_level: usize,

    #[allow(dead_code)]
    #[serde(default = "AndroidConfig::default_package_name")]
    pub(crate) package_name: String,
}

impl AndroidConfig {
    fn default_package_name() -> String {
        // TODO: derive this from package.json.
        "com.example.rust.package".to_string()
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
    fn directory(&self) -> Result<Utf8PathBuf> {
        Ok(workspace::project_root()?.join(&self.directory))
    }

    fn jni_libs(&self) -> Result<Utf8PathBuf> {
        Ok(self.directory()?.join(&self.jni_libs))
    }
}

#[derive(Args, Debug)]
pub(crate) struct AndroidArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    #[clap(flatten)]
    common_args: CommonBuildArgs,
}

impl AndroidArgs {
    pub(crate) fn build(&self) -> Result<()> {
        let config: ProjectConfig = self.config.clone().try_into()?;
        let crate_ = &config.crate_;
        let rust_dir = crate_.directory()?;
        let manifest_path = crate_.manifest_path()?;

        let android = config.android;

        let jni_libs = android.jni_libs()?;
        rm_dir(&jni_libs)?;

        for target in &android.targets {
            let mut cmd = Command::new("cargo");
            cmd.arg("ndk")
                .arg("--manifest-path")
                .arg(&manifest_path)
                .arg("--target")
                .arg(target)
                .arg("--platform")
                .arg(format!("{}", android.api_level))
                .arg("--output-dir")
                .arg(&jni_libs);

            if !self.common_args.release {
                cmd.arg("--no-strip");
            }

            cmd.arg("--").arg("build");
            if self.common_args.release {
                cmd.arg("--release");
            }

            cmd.args(android.cargo_extras.clone());

            run_cmd(cmd.current_dir(&rust_dir))?
        }

        Ok(())
    }
}
