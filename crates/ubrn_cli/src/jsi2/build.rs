/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use crate::{
    config::ProjectConfig,
    jsi2::{android::Jsi2AndroidBuildArgs, ios::Jsi2IosBuildArgs},
};

/// `ubrn build jsi2 {android,ios}`: the crate as the shared library the player
/// loads. Sits under `build` like every other flavour; `--and-generate` runs
/// `generate jsi2 all` on the result through the same path v1 uses.
#[derive(Args, Debug)]
pub(crate) struct BuildArgs {
    #[clap(subcommand)]
    cmd: Jsi2BuildCmd,
}

#[derive(Debug, Subcommand)]
enum Jsi2BuildCmd {
    /// lib<name>.so per ABI into android/src/main/jniLibs
    Android(Jsi2AndroidBuildArgs),
    /// <name>.framework per platform, combined into ios/<name>.xcframework
    Ios(Jsi2IosBuildArgs),
}

impl BuildArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        match &self.cmd {
            Jsi2BuildCmd::Android(a) => a.build(),
            Jsi2BuildCmd::Ios(i) => i.build(),
        }
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        match &self.cmd {
            Jsi2BuildCmd::Android(a) => a.project_config(),
            Jsi2BuildCmd::Ios(i) => i.project_config(),
        }
    }

    pub(crate) fn and_generate(&self) -> bool {
        match &self.cmd {
            Jsi2BuildCmd::Android(a) => a.common_args.and_generate,
            Jsi2BuildCmd::Ios(i) => i.common_args.and_generate,
        }
    }
}
