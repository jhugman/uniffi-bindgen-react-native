/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{
    collections::{BTreeSet, HashMap},
    process::Command,
};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{cp_file, mk_dir, rm_dir, run_cmd, CrateMetadata};
use uniffi_bindgen::{
    bindings::KotlinBindingGenerator, cargo_metadata::CrateConfigSupplier,
    library_mode::generate_bindings,
};

use crate::{
    commands::building::CommonBuildArgs,
    config::{rust_crate::CrateConfig, ExtraArgs, ProjectConfig},
    jsi::android::config::Target,
};

#[derive(Args, Debug)]
pub(crate) struct AndroidBuildArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    /// Comma separated list of targets, that override the values in the
    /// `config.yaml` file.
    ///
    /// Android:
    ///   aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android,i686-linux-android,
    ///
    /// Synonyms for:
    ///   arm64-v8a,armeabi-v7a,x86_64,x86
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub(crate) targets: Vec<Target>,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,

    /// Suppress the copying of the Rust library into the JNI library directories.
    #[clap(long = "no-jniLibs")]
    no_jni_libs: bool,

    /// Generate native Kotlin Bindings together with the JNI libraries
    #[clap(long, conflicts_with_all = ["no_jni_libs"], default_value = "false")]
    native_bindings: bool,
}

impl AndroidBuildArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config: ProjectConfig = self.project_config()?;
        let android = &config.android;
        let target_list = &android.targets;
        let crate_ = &config.crate_;
        let target_files = if self.common_args.no_cargo {
            let files = self.find_existing(&crate_.metadata()?, target_list);
            if !files.is_empty() {
                files
            } else {
                self.cargo_build_all(
                    crate_,
                    target_list,
                    &android.cargo_extras,
                    android.api_level,
                )?
            }
        } else {
            self.cargo_build_all(
                crate_,
                target_list,
                &android.cargo_extras,
                android.api_level,
            )?
        };

        if !self.no_jni_libs {
            let project_root = config.project_root();
            self.copy_into_jni_libs(
                &crate_.metadata()?,
                &android.jni_libs(project_root),
                &target_files,
            )?;
            if self.native_bindings {
                self.generate_native_bindings(&config, &target_files)?;
            }
        }

        Ok(target_files.into_values().collect())
    }

    fn cargo_build_all(
        &self,
        crate_: &CrateConfig,
        targets: &[Target],
        cargo_extras: &ExtraArgs,
        api_level: usize,
    ) -> Result<HashMap<Target, Utf8PathBuf>> {
        let manifest_path = crate_.manifest_path()?;
        let rust_dir = crate_.crate_dir()?;
        let metadata = crate_.metadata()?;
        let mut target_files = HashMap::new();
        let profile = self.common_args.profile();
        for target in targets {
            let target =
                self.cargo_build(target, &manifest_path, cargo_extras, api_level, &rust_dir)?;
            let library = metadata.library_path(Some(target.triple()), profile);
            metadata.library_path_exists(&library)?;
            target_files.insert(target, library);
        }
        Ok(target_files)
    }

    fn cargo_build(
        &self,
        target: &Target,
        manifest_path: &Utf8PathBuf,
        cargo_extras: &ExtraArgs,
        api_level: usize,
        rust_dir: &Utf8PathBuf,
    ) -> Result<Target> {
        let mut cmd = Command::new("cargo");
        cmd.arg("ndk")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--target")
            .arg(target.to_string())
            .arg("--platform")
            .arg(format!("{}", api_level));
        let profile = self.common_args.profile();
        if profile != "release" {
            cmd.arg("--no-strip");
        }
        cmd.arg("--").arg("build");
        if profile != "debug" {
            cmd.args(["--profile", profile]);
        }
        cmd.args(cargo_extras.clone());
        run_cmd(cmd.current_dir(rust_dir))?;
        Ok(target.clone())
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
                let library = metadata.library_path(Some(target.triple()), profile);
                if library.exists() {
                    Some((target.clone(), library))
                } else {
                    None
                }
            })
            .collect()
    }

    fn copy_into_jni_libs(
        &self,
        metadata: &CrateMetadata,
        jni_libs: &Utf8Path,
        target_files: &HashMap<Target, Utf8PathBuf>,
    ) -> Result<()> {
        println!("-- Copying into jniLibs directory");
        println!("rm -Rf {jni_libs}");
        rm_dir(jni_libs)?;
        for (target, library) in target_files {
            let dst_dir = jni_libs.join(target.to_string());
            mk_dir(&dst_dir)?;

            let dst_lib = dst_dir.join(metadata.library_file(Some(target.triple())));
            println!("cp {library} {dst_lib}");
            cp_file(library, &dst_lib)?;
        }
        Ok(())
    }

    fn generate_native_bindings(
        &self,
        config: &ProjectConfig,
        target_files: &HashMap<Target, Utf8PathBuf>,
    ) -> Result<(), anyhow::Error> {
        println!("-- Generating native Kotlin bindings");
        let manifest_path = config.crate_.manifest_path()?;
        let metadata = CrateMetadata::cargo_metadata(manifest_path)?;
        let config_supplier = CrateConfigSupplier::from(metadata);
        let libs = target_files
            .clone()
            .into_values()
            .collect::<BTreeSet<Utf8PathBuf>>();
        let library_path = libs
            .first()
            .context("Need at least one library file to generate native Kotlin bindings")?;
        let out_dir = config.android.src_main_java_dir(config.project_root());
        generate_bindings(
            library_path,
            None,
            &KotlinBindingGenerator,
            &config_supplier,
            None,
            &out_dir,
            false,
        )?;
        Ok(())
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let mut config: ProjectConfig = self.config.clone().try_into()?;
        let android = &mut config.android;
        if !self.targets.is_empty() {
            android.targets = self.targets.clone();
        }
        Ok(config)
    }
}
