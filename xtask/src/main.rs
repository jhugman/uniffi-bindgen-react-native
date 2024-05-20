/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use clap::{Parser, Subcommand};
use fmt::FmtArgs;
use run::RunCmd;

use crate::{bootstrap::BootstrapCmd, clean::CleanCmd};

mod bootstrap;
mod clean;
mod fmt;
mod run;
mod util;

#[derive(Debug, Parser)]
struct CliArgs {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Prepare the directory for development
    Bootstrap(BootstrapCmd),
    /// Remove everything as if just git cloned
    Clean(CleanCmd),
    /// Run some Javascript against a Rust library.
    ///
    /// Optionally can compile the Rust.
    Run(RunCmd),

    /// Format all code in the repository
    Fmt(FmtArgs),
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Cmd::Bootstrap(c) => c.run(),
        Cmd::Clean(c) => c.run(),
        Cmd::Run(c) => c.run(),
        Cmd::Fmt(c) => c.run(),
    }
}
