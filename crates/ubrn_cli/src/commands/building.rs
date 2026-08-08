/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use ubrn_common::CrateMetadata;

#[cfg(feature = "wasm")]
use crate::wasm::WebBuildArgs;
#[cfg(feature = "wasm")]
use crate::wasm2::Wasm2BuildArgs;
use crate::{
    commands::generate::GenerateAllCommand, config::ProjectConfig, jsi::android::AndroidBuildArgs,
    jsi::ios::IosBuildArgs, Platform,
};

#[derive(Args, Debug)]
pub(crate) struct BuildArgs {
    #[clap(subcommand)]
    cmd: BuildCmd,
}

#[derive(Subcommand, Debug)]
pub(crate) enum BuildCmd {
    /// Build the crate for use on an Android device or emulator
    Android(AndroidBuildArgs),
    /// Build the crate for use on an iOS device or simulator
    Ios(IosBuildArgs),
    /// Build the crate for use in a web page
    #[cfg(feature = "wasm")]
    #[clap(aliases = ["wasm"])]
    Web(WebBuildArgs),
    /// Build the crate for the Wasm2 (player-based) runtime
    #[cfg(feature = "wasm")]
    #[clap(aliases = ["web2"])]
    Wasm2(Wasm2BuildArgs),
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
        eprintln!("Generating bindings and turbo module from lib file {lib_file}");
        GenerateAllCommand::platform_specific(
            lib_file,
            self.cmd.project_config()?,
            Platform::from(&self.cmd),
            self.cmd.native_bindings(),
        )
        .run()?;

        self.cmd.then_build()
    }
}

impl BuildCmd {
    pub(crate) fn build(&self) -> Result<Utf8PathBuf> {
        let mut files = match self {
            Self::Android(a) => a.build()?,
            Self::Ios(a) => a.build()?,
            #[cfg(feature = "wasm")]
            Self::Web(a) => a.build()?,
            #[cfg(feature = "wasm")]
            Self::Wasm2(a) => a.build()?,
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
            #[cfg(feature = "wasm")]
            Self::Web(a) => a.project_config(),
            #[cfg(feature = "wasm")]
            Self::Wasm2(a) => a.project_config(),
        }
    }

    pub(crate) fn and_generate(&self) -> bool {
        match self {
            Self::Android(a) => a.common_args.and_generate,
            Self::Ios(a) => a.common_args.and_generate,
            #[cfg(feature = "wasm")]
            Self::Web(a) => !a.no_generate,
            #[cfg(feature = "wasm")]
            Self::Wasm2(a) => !a.no_generate,
        }
    }

    #[cfg(feature = "wasm")]
    pub(crate) fn then_build(&self) -> Result<()> {
        match self {
            Self::Web(a) => {
                if !a.no_wasm_pack {
                    a.then_build()?
                }
            }
            // Wasm2 built its wasm in `build()` — this stages it next to the
            // TypeScript that `generate` has just written.
            Self::Wasm2(a) => a.then_build()?,
            _ => {}
        }
        Ok(())
    }

    #[cfg(not(feature = "wasm"))]
    pub(crate) fn then_build(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) fn native_bindings(&self) -> bool {
        match self {
            Self::Android(a) => a.native_bindings,
            Self::Ios(a) => a.native_bindings,
            #[cfg(feature = "wasm")]
            Self::Web(_) => false, // Web does not support native bindings
            #[cfg(feature = "wasm")]
            Self::Wasm2(_) => false, // Wasm2 does not support native bindings
        }
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

impl From<&BuildCmd> for Platform {
    fn from(value: &BuildCmd) -> Self {
        match value {
            BuildCmd::Android(..) => Self::Android,
            BuildCmd::Ios(..) => Self::Ios,
            #[cfg(feature = "wasm")]
            BuildCmd::Web(..) => Self::Wasm,
            #[cfg(feature = "wasm")]
            BuildCmd::Wasm2(..) => Self::Wasm2,
        }
    }
}
