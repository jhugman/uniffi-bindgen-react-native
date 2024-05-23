/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod bindings;

use anyhow::Result;
use clap::{Parser, Subcommand};

use bindings::BindingsArgs;

#[derive(Parser)]
struct CliArgs {
    #[command(subcommand)]
    cmd: CliCmd,
}

#[derive(Subcommand, Debug)]
enum CliCmd {
    /// Generates bindings from a Rust crate or UDL file
    Bindings(BindingsArgs),
}

impl CliCmd {
    fn run(&self) -> Result<()> {
        match self {
            Self::Bindings(a) => a.run(),
        }
    }
}

fn main() -> Result<()> {
    let args = CliArgs::parse();
    args.cmd.run()
}
