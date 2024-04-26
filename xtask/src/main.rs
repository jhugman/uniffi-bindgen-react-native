use anyhow::Result;
use clap::{Parser, Subcommand};
use run::RunCmd;

use crate::{bootstrap::BootstrapCmd, clean::CleanCmd};

mod bootstrap;
mod clean;
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
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Cmd::Bootstrap(c) => c.run(),
        Cmd::Clean(c) => c.run(),
        Cmd::Run(c) => c.run(),
    }
}
