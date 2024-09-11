/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{collections::HashMap, process::Command};

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use heck::ToUpperCamelCase;
use serde::Deserialize;
use ubrn_common::{mk_dir, rm_dir, run_cmd, CrateMetadata};

use crate::{
    building::{CommonBuildArgs, ExtraArgs},
    config::ProjectConfig,
    rust::CrateConfig,
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

    /// Does not perform the xcodebuild step to generate the xcframework
    ///
    /// The xcframework will need to be generated externally from this tool.
    /// This is useful when adding extra bindings (e.g. Swift) to the project.
    #[clap(long, alias = "no-xcframework")]
    no_xcodebuild: bool,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl IOsArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        let ios = &config.ios;

        let targets = ios
            .targets
            .iter()
            .map(|t| {
                TARGETS
                    .iter()
                    .find(|target| target.triple == *t)
                    .unwrap_or_else(|| panic!("Invalid target specified: {t}"))
            })
            .filter(|target| {
                let is_sim = target.platform == Platform::IosSimulator;
                if self.no_sim {
                    !is_sim
                } else if self.sim_only {
                    is_sim
                } else {
                    true
                }
            })
            .cloned()
            .collect::<Vec<_>>();

        let target_files = if self.common_args.no_cargo {
            let files = self.find_existing(&crate_.metadata()?, &targets);
            if !files.is_empty() {
                files
            } else {
                self.cargo_build_all(crate_, &targets, &ios.cargo_extras)?
            }
        } else {
            self.cargo_build_all(crate_, &targets, &ios.cargo_extras)?
        };

        Ok(if !self.no_xcodebuild {
            let target_files = self.lipo_when_necessary(crate_, target_files)?;
            self.create_xcframework(&config, &target_files)?;
            target_files
        } else {
            target_files.into_values().collect()
        })
    }

    fn cargo_build_all(
        &self,
        crate_: &CrateConfig,
        targets: &[Target],
        cargo_extras: &ExtraArgs,
    ) -> Result<HashMap<Target, Utf8PathBuf>> {
        let mut target_files = HashMap::new();
        let metadata = crate_.metadata()?;
        let rust_dir = crate_.directory()?;
        let manifest_path = crate_.manifest_path()?;
        for target in targets {
            self.cargo_build(&manifest_path, target, cargo_extras, &rust_dir)?;

            // Now we need to get the path to the lib.a file, to feed to xcodebuild.
            let library = metadata.library_path(Some(target.triple), self.common_args.profile());
            metadata.library_path_exists(&library)?;
            target_files.insert(target.clone(), library);
        }
        Ok(target_files)
    }

    fn cargo_build(
        &self,
        manifest_path: &Utf8PathBuf,
        target: &Target,
        cargo_extras: &ExtraArgs,
        rust_dir: &Utf8PathBuf,
    ) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--target")
            .arg(target.triple);
        if self.common_args.release {
            cmd.arg("--release");
        }
        cmd.args(cargo_extras.clone());
        run_cmd(cmd.current_dir(rust_dir))?;
        Ok(())
    }

    fn lipo_when_necessary(
        &self,
        crate_: &CrateConfig,
        target_files: HashMap<Target, Utf8PathBuf>,
    ) -> Result<Vec<Utf8PathBuf>> {
        let mut by_platform = HashMap::new();
        for (target, file) in target_files {
            let files = by_platform.entry(target.platform).or_insert(Vec::new());
            files.push(file);
        }
        let metadata = crate_.metadata()?;

        let mut sorted = Vec::new();
        for (p, mut files) in by_platform {
            if files.len() == 1 {
                sorted.append(&mut files);
            } else {
                let dir = metadata.target_dir().join("lipo").join(p.lib_folder_name());
                mk_dir(&dir)?;
                let output = dir.join(metadata.library_file(Some("ios")));
                let mut cmd = Command::new("lipo");
                cmd.arg("-create");
                for f in files {
                    cmd.arg(f);
                }
                cmd.arg("-output").arg(&output);
                run_cmd(&mut cmd)?;
                sorted.push(output);
            }
        }

        Ok(sorted)
    }

    fn create_xcframework(
        &self,
        config: &ProjectConfig,
        target_files: &[Utf8PathBuf],
    ) -> Result<(), anyhow::Error> {
        let ios = &config.ios;
        let project_root = config.project_root();
        let ios_dir = ios.directory(project_root);
        let mut library_args = Vec::new();
        for library in target_files {
            // :eyes: single dash arg.
            library_args.push("-library".to_string());
            library_args.push(library.to_string());
        }
        let framework_path = ios.framework_path(project_root);
        if framework_path.exists() {
            rm_dir(&framework_path)?;
        }
        let mut cmd = Command::new("xcodebuild");
        cmd.arg("-create-xcframework")
            .args(library_args)
            .arg("-output")
            .arg(&framework_path)
            .args(ios.xcodebuild_extras.clone());
        run_cmd(cmd.current_dir(ios_dir))?;
        Ok(())
    }

    fn find_existing(
        &self,
        metadata: &CrateMetadata,
        targets: &[Target],
    ) -> HashMap<Target, Utf8PathBuf> {
        let profile = self.common_args.profile();
        targets
            .iter()
            .filter_map(|target| {
                let library = metadata.library_path(Some(target.triple), profile);
                if library.exists() {
                    Some((target.clone(), library))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>()
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        self.config.clone().try_into()
    }

    pub(crate) fn config(&self) -> Utf8PathBuf {
        self.config.clone()
    }
}

/// A specific build target supported by the SDK.
#[derive(Hash, PartialEq, Eq, Clone)]
struct Target {
    triple: &'static str,
    platform: Platform,
    description: &'static str,
}

/// The platform for which a particular target can run on.
#[derive(Hash, PartialEq, Eq, Clone)]
enum Platform {
    Macos,
    Ios,
    IosSimulator,
}

impl Platform {
    /// The name of the subfolder in which to place the library for the platform
    /// once all architectures are lipo'd together.
    fn lib_folder_name(&self) -> &str {
        match self {
            Platform::Macos => "macos",
            Platform::Ios => "ios",
            Platform::IosSimulator => "ios-simulator",
        }
    }
}

/// The list of targets supported by the SDK.
const TARGETS: &[Target] = &[
    Target {
        triple: "aarch64-apple-ios",
        platform: Platform::Ios,
        description: "iOS",
    },
    Target {
        triple: "aarch64-apple-darwin",
        platform: Platform::Macos,
        description: "macOS (Apple Silicon)",
    },
    Target {
        triple: "x86_64-apple-darwin",
        platform: Platform::Macos,
        description: "macOS (Intel)",
    },
    Target {
        triple: "aarch64-apple-ios-sim",
        platform: Platform::IosSimulator,
        description: "iOS Simulator (Apple Silicon)",
    },
    Target {
        triple: "x86_64-apple-ios",
        platform: Platform::IosSimulator,
        description: "iOS Simulator (Intel) ",
    },
];
