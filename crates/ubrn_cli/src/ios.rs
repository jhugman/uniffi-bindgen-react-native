/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use heck::ToUpperCamelCase;
use serde::Deserialize;
use ubrn_common::{rm_dir, run_cmd};

use crate::{
    building::{CommonBuildArgs, ExtraArgs},
    config::ProjectConfig,
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
    pub(crate) targets: Vec<String>,

    #[serde(default = "IOsConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,
}

impl IOsConfig {
    fn default_directory() -> String {
        "ios".to_string()
    }

    fn default_framework_name() -> String {
        format!(
            "{}Framework",
            workspace::package_json().name().to_upper_camel_case()
        )
    }

    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }

    fn default_xcodebuild_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }

    fn default_targets() -> Vec<String> {
        let args: &[&str] = &["aarch64-apple-ios", "aarch64-apple-ios-sim"];
        args.iter().map(|s| s.to_string()).collect()
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

    pub(crate) fn framework_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = format!("{}.xcframework", self.framework_name);
        project_root.join(filename)
    }
}

#[derive(Args, Debug)]
pub(crate) struct IOsArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    /// Only build for the simulator
    #[clap(long, default_value = "false")]
    sim_only: bool,

    /// Exclude builds for the simulator
    #[clap(long, conflicts_with_all = ["sim_only"], default_value = "false")]
    no_sim: bool,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl IOsArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let project_root = config.project_root();
        let crate_ = &config.crate_;
        let metadata = crate_.metadata()?;
        let rust_dir = crate_.directory()?;
        let profile = self.common_args.profile();
        let manifest_path = crate_.manifest_path()?;

        let ios = &config.ios;
        let ios_dir = ios.directory(project_root);

        let mut library_args = Vec::new();
        let mut target_files = Vec::new();
        for target in &ios.targets {
            if self.no_sim && target.contains("sim") {
                continue;
            }
            if self.sim_only && !target.contains("sim") {
                continue;
            }

            let mut cmd = Command::new("cargo");
            cmd.arg("build")
                .arg("--manifest-path")
                .arg(&manifest_path)
                .arg("--target")
                .arg(target);
            if self.common_args.release {
                cmd.arg("--release");
            }

            cmd.args(ios.cargo_extras.clone());
            run_cmd(cmd.current_dir(&rust_dir))?;

            // Now we need to get the path to the lib.a file,
            // to feed to xcodebuild.
            let library = metadata.library_path(Some(target), profile)?;
            if !library.exists() {
                anyhow::bail!("Calculated library doesn't exist. This may be because `staticlib` is not in the `crate_type` list in the [[lib]] entry of Cargo.toml.");
            }
            // :eyes: single dash arg.
            library_args.push("-library".to_string());
            library_args.push(library.to_string());
            target_files.push(library);
        }

        let framework_path = ios.framework_path(project_root);
        if framework_path.exists() {
            rm_dir(&framework_path)?;
        }
        let mut cmd = Command::new("xcodebuild");
        // :eyes: single dash arg.
        cmd.arg("-create-xcframework")
            .args(library_args)
            .arg("-output")
            .arg(&framework_path)
            .args(ios.xcodebuild_extras.clone());

        run_cmd(cmd.current_dir(ios_dir))?;

        Ok(target_files)
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        self.config.clone().try_into()
    }
}
