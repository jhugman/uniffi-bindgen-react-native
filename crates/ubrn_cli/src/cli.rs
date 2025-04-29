/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use crate::{
    commands::{checkout::GitRepoArgs, BuildArgs, CheckoutArgs, GenerateArgs},
    workspace, AsConfig,
};
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
pub(crate) struct CliArgs {
    #[command(subcommand)]
    pub(crate) cmd: CliCmd,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CliCmd {
    /// Checkout a given Github repo into `rust_modules`
    Checkout(CheckoutArgs),
    /// Build (and optionally generate code) for Android or iOS
    Build(BuildArgs),
    /// Generate bindings or the turbo-module glue code from the Rust.
    ///
    /// These steps are already performed when building with `--and-generate`.
    Generate(GenerateArgs),
}

impl CliCmd {
    pub(crate) fn run(&self) -> Result<()> {
        match self {
            Self::Checkout(c) => {
                AsConfig::<GitRepoArgs>::as_config(c)?.checkout(&workspace::project_root()?)
            }
            Self::Build(b) => b.build(),
            Self::Generate(g) => g.run(),
        }
    }
}
