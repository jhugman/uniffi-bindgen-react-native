/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use clap::{Parser, Subcommand};
use fmt::FmtArgs;

use crate::{bootstrap::BootstrapCmd, clean::CleanCmd};

mod bootstrap;
mod clean;
mod fmt;
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
    /// Format all code in the repository
    Fmt(FmtArgs),
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Cmd::Bootstrap(c) => c.run(),
        Cmd::Clean(c) => c.run(),
        Cmd::Fmt(c) => c.run(),
    }
}

#[cfg(test)]
mod cli_test {
    use clap::Parser;

    use super::*;

    fn parse(args: &[&str]) -> CliArgs {
        let mut all_args = vec![""];
        all_args.extend_from_slice(args);

        CliArgs::parse_from(all_args)
    }

    #[test]
    fn test_bootstrap_command() {
        assert!(matches!(parse(&["bootstrap"]).cmd, Cmd::Bootstrap(_)));
    }
}
