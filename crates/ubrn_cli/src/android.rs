/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, fs, process::Command, str::FromStr};

use clap::Args;

use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use ubrn_common::{mk_dir, rm_dir, run_cmd, CrateMetadata};

use crate::{
    building::{CommonBuildArgs, ExtraArgs},
    config::ProjectConfig,
    rust::CrateConfig,
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
    pub(crate) targets: Vec<Target>,

    #[serde(default = "AndroidConfig::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(default = "AndroidConfig::default_platform", alias = "platform")]
    pub(crate) api_level: usize,

    #[serde(default = "AndroidConfig::default_package_name")]
    pub(crate) package_name: String,

    #[serde(default = "AndroidConfig::default_codegen_output_dir")]
    pub(crate) codegen_output_dir: String,
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

#[derive(Args, Debug)]
pub(crate) struct AndroidArgs {
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
}

impl AndroidArgs {
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
        if !self.common_args.release {
            cmd.arg("--no-strip");
        }
        cmd.arg("--").arg("build");
        if self.common_args.release {
            cmd.arg("--release");
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
            fs::copy(library, &dst_lib)?;
        }
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
