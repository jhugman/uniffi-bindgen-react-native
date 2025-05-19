/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{collections::HashMap, process::Command};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{mk_dir, mv_files, rm_dir, run_cmd, CrateMetadata};
use uniffi_bindgen::{
    bindings::SwiftBindingGenerator, cargo_metadata::CrateConfigSupplier,
    library_mode::generate_bindings,
};

use crate::{
    commands::building::CommonBuildArgs,
    config::{rust_crate::CrateConfig, ExtraArgs, ProjectConfig},
    jsi::ios::config::{Platform, Target},
};

#[derive(Args, Debug)]
pub(crate) struct IosBuildArgs {
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

    /// Generate native Swift Bindings together with the xcframework
    #[clap(long, conflicts_with_all = ["no_xcodebuild"], default_value = "false")]
    native_bindings: bool,

    /// Comma separated list of targets, that override the values in
    /// the `config.yaml` file.
    ///
    /// iOS:
    ///  aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub(crate) targets: Vec<Target>,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl IosBuildArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        let ios = &config.ios;
        let target_list = &ios.targets;

        let targets = target_list
            .iter()
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
            let mut target_files = self.lipo_when_necessary(crate_, target_files)?;
            target_files.sort();
            if self.native_bindings {
                self.generate_native_bindings(&config, &target_files)?;
            }
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
            let library = metadata.library_path(Some(&target.triple), self.common_args.profile());
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
            .arg(&target.triple);
        let profile = self.common_args.profile();
        if profile != "debug" {
            cmd.args(["--profile", profile]);
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

    fn generate_native_bindings(
        &self,
        config: &ProjectConfig,
        target_files: &[Utf8PathBuf],
    ) -> Result<(), anyhow::Error> {
        let manifest_path = config.crate_.manifest_path()?;
        let metadata = CrateMetadata::cargo_metadata(manifest_path)?;
        let config_supplier = CrateConfigSupplier::from(metadata);
        let library_path = target_files
            .first()
            .context("Need at least one library file to generate native iOS bindings")?;
        let out_dir = config.ios.native_bindings_dir(config.project_root());
        if out_dir.exists() {
            rm_dir(&out_dir)?;
        }
        ubrn_common::mk_dir(&out_dir)?;
        generate_bindings(
            library_path,
            None,
            &SwiftBindingGenerator,
            &config_supplier,
            None,
            &out_dir,
            false,
        )?;
        Ok(())
    }

    fn create_xcframework(
        &self,
        config: &ProjectConfig,
        target_files: &[Utf8PathBuf],
    ) -> Result<(), anyhow::Error> {
        let ios = &config.ios;
        let project_root = config.project_root();
        let ios_dir = ios.directory(project_root);
        ubrn_common::mk_dir(&ios_dir)?;
        let native_bindings_dir = ios.native_bindings_dir(project_root);
        let header_dir = native_bindings_dir.join("headers");
        if self.native_bindings {
            ubrn_common::mk_dir(&header_dir)?;
            mv_files("h", &native_bindings_dir, &header_dir)?;
            self.merge_modulemaps(&native_bindings_dir, &header_dir.join("module.modulemap"))?;
        }
        let mut library_args = Vec::new();
        for library in target_files {
            // :eyes: single dash arg.
            library_args.push("-library".to_string());
            library_args.push(library.to_string());
            if self.native_bindings {
                library_args.push("-headers".to_string());
                library_args.push(header_dir.to_string());
            }
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
        if self.native_bindings {
            rm_dir(&header_dir)?;
        }
        Ok(())
    }

    fn merge_modulemaps(&self, dir: &Utf8Path, out_file: &Utf8Path) -> Result<()> {
        let mut contents = String::new();
        for entry in dir.read_dir_utf8()? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() || path.extension() != Some("modulemap") {
                continue;
            }
            let chunk = ubrn_common::read_to_string(path)?;
            contents.push_str(&chunk);
            contents.push_str("\n\n");
            ubrn_common::rm_file(path)?;
        }
        ubrn_common::write_file(out_file, contents)?;
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
                let library = metadata.library_path(Some(&target.triple), profile);
                if library.exists() {
                    Some((target.clone(), library))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>()
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let mut config: ProjectConfig = self.config.clone().try_into()?;
        let ios = &mut config.ios;
        if !self.targets.is_empty() {
            ios.targets = self.targets.clone();
        }
        Ok(config)
    }
}
