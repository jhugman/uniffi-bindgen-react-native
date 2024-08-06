/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use serde::Deserialize;
use ubrn_bindgen::{BindingsArgs, OutputArgs, SourceArgs};
use ubrn_common::CrateMetadata;

use crate::{android::AndroidArgs, config::ProjectConfig, ios::IOsArgs};

#[derive(Args, Debug)]
pub(crate) struct BuildArgs {
    #[clap(subcommand)]
    cmd: BuildCmd,
}

#[derive(Subcommand, Debug)]
pub(crate) enum BuildCmd {
    /// Build the crate for use on an Android device or emulator
    Android(AndroidArgs),
    /// Build the crate for use on an iOS device or simulator
    Ios(IOsArgs),
}

impl BuildArgs {
    pub(crate) fn build(&self) -> Result<()> {
        let lib_file = self.cmd.build()?;
        if self.and_generate() {
            self.generate(lib_file)?;
        }

        Ok(())
    }

    fn generate(&self, lib_file: Utf8PathBuf) -> Result<()> {
        let project = self.cmd.project_config()?;
        let root = project.project_root();
        let pwd = ubrn_common::pwd()?;
        let modules = {
            let dir = project.crate_.directory()?;
            ubrn_common::cd(&dir)?;
            let ts_dir = project.bindings.ts_path(root);
            let cpp_dir = project.bindings.cpp_path(root);
            let config = project.bindings.uniffi_toml_path(root);
            if let Some(ref file) = config {
                if !file.exists() {
                    anyhow::bail!("uniffi.toml file {:?} does not exist. Either delete the uniffiToml property or supply a file", file)
                }
            }
            let bindings = BindingsArgs::new(
                SourceArgs::library(&lib_file).with_config(config),
                OutputArgs::new(&ts_dir, &cpp_dir, false),
            );

            bindings.run()?
        };
        ubrn_common::cd(&pwd)?;

        let rust_crate = project.crate_.metadata()?;
        crate::codegen::render_files(project, rust_crate, modules)?;
        Ok(())
    }

    fn and_generate(&self) -> bool {
        self.cmd.and_generate()
    }
}

impl BuildCmd {
    pub(crate) fn build(&self) -> Result<Utf8PathBuf> {
        let files = match self {
            Self::Android(a) => a.build()?,
            Self::Ios(a) => a.build()?,
        };

        files
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("No targets were specified"))
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        match self {
            Self::Android(a) => a.project_config(),
            Self::Ios(a) => a.project_config(),
        }
    }

    fn common_args(&self) -> &CommonBuildArgs {
        match self {
            Self::Android(a) => &a.common_args,
            Self::Ios(a) => &a.common_args,
        }
    }

    pub(crate) fn and_generate(&self) -> bool {
        self.common_args().and_generate
    }
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CommonBuildArgs {
    /// Build a release build
    #[clap(long, short, default_value = "false")]
    pub(crate) release: bool,

    /// Optionally generate the bindings and turbo-module code for the crate
    #[clap(long = "and-generate", short = 'g')]
    pub(crate) and_generate: bool,
}

impl CommonBuildArgs {
    pub(crate) fn profile<'a>(&self) -> &'a str {
        CrateMetadata::profile(self.release)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ExtraArgs {
    AsList(Vec<String>),
    AsString(String),
}

impl IntoIterator for ExtraArgs {
    type Item = String;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ExtraArgs::AsList(v) => v.into_iter(),
            ExtraArgs::AsString(s) => s
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .into_iter(),
        }
    }
}

impl Default for ExtraArgs {
    fn default() -> Self {
        Self::AsList(Default::default())
    }
}

impl From<&[&str]> for ExtraArgs {
    fn from(value: &[&str]) -> Self {
        let vec = value.iter().map(|&s| s.to_string()).collect();
        ExtraArgs::AsList(vec)
    }
}
