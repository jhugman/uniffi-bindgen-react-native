/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Deserialize;
use ubrn_common::CrateMetadata;

use crate::{android::AndroidArgs, ios::IOsArgs};

#[derive(Args, Debug)]
pub(crate) struct BuildArgs {
    #[clap(subcommand)]
    cmd: BuildCmd,
}

#[derive(Subcommand, Debug)]
pub(crate) enum BuildCmd {
    Android(AndroidArgs),
    Ios(IOsArgs),
}

impl BuildArgs {
    pub(crate) fn build(&self) -> Result<()> {
        self.cmd.build()
    }
}

impl BuildCmd {
    pub(crate) fn build(&self) -> Result<()> {
        match self {
            Self::Android(a) => a.build(),
            Self::Ios(a) => a.build(),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CommonBuildArgs {
    /// Build a release build
    #[clap(long, short, default_value = "false")]
    pub(crate) release: bool,
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
