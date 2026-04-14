/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use ubrn_bindgen::{AbiFlavor, OutputArgs, SourceArgs, SwitchArgs};

#[derive(Args, Debug)]
pub(crate) struct CmdArg {
    #[clap(subcommand)]
    cmd: Cmd,
}

impl CmdArg {
    pub(crate) fn run(&self) -> Result<()> {
        self.cmd.run()
    }
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Generate just the Typescript bindings for N-API
    Bindings(BindingsArgs),
}

impl Cmd {
    fn run(&self) -> Result<()> {
        match self {
            Self::Bindings(b) => {
                let b = ubrn_bindgen::BindingsArgs::from(b);
                b.run(None)?;
                Ok(())
            }
        }
    }
}

#[derive(Args, Debug)]
pub(crate) struct BindingsArgs {
    #[command(flatten)]
    pub(crate) source: SourceArgs,

    /// By default, bindgen will attempt to format the code with prettier.
    #[clap(long)]
    pub(crate) no_format: bool,

    /// The directory in which to put the generated Typescript.
    #[clap(long)]
    pub(crate) ts_dir: Utf8PathBuf,
}

impl From<&BindingsArgs> for ubrn_bindgen::BindingsArgs {
    fn from(value: &BindingsArgs) -> Self {
        // Napi doesn't generate C++, so we pass ts_dir as a dummy for cpp_dir.
        ubrn_bindgen::BindingsArgs::new(
            SwitchArgs {
                flavor: AbiFlavor::Napi,
            },
            value.source.clone(),
            OutputArgs::new(&value.ts_dir, &value.ts_dir, value.no_format),
        )
    }
}
