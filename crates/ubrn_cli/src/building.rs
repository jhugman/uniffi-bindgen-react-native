/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use serde::Deserialize;
use ubrn_common::CrateMetadata;

use crate::{
    android::AndroidArgs, config::ProjectConfig, generate::GenerateAllCommand, ios::IOsArgs,
    Platform,
};

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
        if self.cmd.and_generate() {
            self.generate(lib_file)?;
        }

        Ok(())
    }

    fn generate(&self, lib_file: Utf8PathBuf) -> Result<()> {
        eprintln!(
            "Generating bindings and turbo module from lib file {}",
            lib_file
        );
        GenerateAllCommand::platform_specific(
            lib_file,
            self.cmd.project_config()?,
            Platform::from(&self.cmd),
        )
        .run()
    }
}

impl BuildCmd {
    pub(crate) fn build(&self) -> Result<Utf8PathBuf> {
        let mut files = match self {
            Self::Android(a) => a.build()?,
            Self::Ios(a) => a.build()?,
        };

        files.sort(); // Sort so that we reproducibly pick the same file below

        files
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("No targets were specified"))
    }

    fn project_config(&self) -> Result<ProjectConfig> {
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

    /// Use a specific build profile
    ///
    /// This overrides the -r / --release flag if both are specified.
    #[clap(long, short)]
    pub(crate) profile: Option<String>,

    /// If the Rust library has been built for at least one target, then
    /// don't re-run cargo build.
    ///
    /// This may be useful if you are using a pre-built library or are
    /// managing the build process yourself.
    #[clap(long)]
    pub(crate) no_cargo: bool,

    /// Optionally generate the bindings and turbo-module code for the crate
    #[clap(long = "and-generate", short = 'g')]
    pub(crate) and_generate: bool,
}

impl CommonBuildArgs {
    pub(crate) fn profile(&self) -> &str {
        CrateMetadata::profile(self.profile.as_deref(), self.release)
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

impl From<&BuildCmd> for Platform {
    fn from(value: &BuildCmd) -> Self {
        match value {
            BuildCmd::Android(..) => Self::Android,
            BuildCmd::Ios(..) => Self::Ios,
        }
    }
}
