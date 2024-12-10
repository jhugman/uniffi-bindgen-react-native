/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
pub(crate) mod cpp_bindings;
pub(crate) mod generate_bindings;
pub(crate) mod jsi;
pub(crate) mod nodejs;
pub(crate) mod rust_crate;
pub(crate) mod typescript;
pub(crate) mod wasm;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use generate_bindings::GenerateBindingsArg;
use jsi::Jsi;
use nodejs::NodeJs;
use ubrn_bindgen::{AbiFlavor, SwitchArgs};
use wasm::Wasm;

use self::{rust_crate::CrateArg, typescript::EntryArg};

#[derive(Debug, Args)]
pub(crate) struct RunCmd {
    /// Clean the crate before starting.
    #[clap(long, short = 'c')]
    pub(crate) clean: bool,

    /// The crate to be bound to hermes
    #[command(flatten)]
    pub(crate) crate_: Option<CrateArg>,

    #[clap(long = "cpp", conflicts_with_all = ["ts_dir", "cpp_dir"])]
    pub(crate) cpp_binding: Option<Utf8PathBuf>,

    #[clap(flatten)]
    pub(crate) generate_bindings: Option<GenerateBindingsArg>,

    #[clap(flatten)]
    pub(crate) switches: SwitchArgs,

    /// The Javascript or Typescript file.
    #[command(flatten)]
    pub(crate) js_file: EntryArg,
}

impl RunCmd {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.switches.flavor {
            AbiFlavor::Jsi => Jsi.run(self),
            AbiFlavor::Wasm => Wasm.run(self),
        }
    }
}
