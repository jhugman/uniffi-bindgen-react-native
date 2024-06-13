/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use clap::{Args, Subcommand};
use ubrn_bindgen::BindingsArgs;

use crate::codegen::TurboModuleArgs;

#[derive(Args, Debug)]
pub(crate) struct GenerateArgs {
    #[clap(subcommand)]
    cmd: GenerateCmd,
}

impl GenerateArgs {
    pub(crate) fn run(&self) -> Result<()> {
        self.cmd.run()
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum GenerateCmd {
    /// Generate the just the bindings
    Bindings(BindingsArgs),
    /// Generate the TurboModule code to plug the bindings into the app
    TurboModule(TurboModuleArgs),
}

impl GenerateCmd {
    pub(crate) fn run(&self) -> Result<()> {
        match self {
            Self::Bindings(b) => {
                b.run()?;
                Ok(())
            }
            Self::TurboModule(t) => {
                t.run()?;
                Ok(())
            }
        }
    }
}
