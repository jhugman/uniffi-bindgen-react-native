/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod cpp_bindings;
mod generate_bindings;
mod rust_crate;
mod typescript;

use anyhow::{Ok, Result};
use camino::Utf8PathBuf;
use clap::Args;
use generate_bindings::GenerateBindingsArg;
use ubrn_common::CrateMetadata;

use crate::bootstrap::{Bootstrap, TestRunnerCmd};

use self::{cpp_bindings::CppBindingArg, rust_crate::CrateArg, typescript::EntryArg};

#[derive(Debug, Args)]
pub(crate) struct RunCmd {
    /// Clean the crate before starting.
    #[clap(long, short = 'c')]
    clean: bool,

    /// The crate to be bound to hermes
    #[command(flatten)]
    crate_: Option<CrateArg>,

    #[clap(long = "cpp", conflicts_with_all = ["ts_dir", "cpp_dir"])]
    cpp_binding: Option<Utf8PathBuf>,

    #[clap(flatten)]
    generate_bindings: Option<GenerateBindingsArg>,

    /// The Javascript or Typescript file.
    #[command(flatten)]
    js_file: EntryArg,
}

impl RunCmd {
    pub(crate) fn run(&self) -> Result<()> {
        TestRunnerCmd.ensure_ready()?;
        let so_file = self.prepare_library_path()?;

        let js_file = self.js_file.prepare()?;
        TestRunnerCmd.run(&js_file, so_file.as_ref())?;
        Ok(())
    }

    fn prepare_library_path(&self) -> Result<Option<Utf8PathBuf>> {
        let clean = self.clean;
        let (release, info) = if let Some(crate_) = &self.crate_ {
            (
                CrateMetadata::profile(crate_.release),
                Some(crate_.cargo_build(clean)?),
            )
        } else {
            (CrateMetadata::profile(false), None)
        };

        match (&info, &self.cpp_binding, &self.generate_bindings) {
            (Some(crate_), Some(cpp), _) => {
                let target_dir = crate_.target_dir();
                let lib_name = crate_.library_name();
                let cpp = CppBindingArg::with_file(cpp.clone());
                let so_file = cpp.compile_with_crate(clean, target_dir, lib_name)?;
                Ok(Some(so_file))
            }
            (None, Some(cpp), _) => {
                let cpp = CppBindingArg::with_file(cpp.clone());
                let so_file = cpp.compile_without_crate(clean)?;
                Ok(Some(so_file))
            }
            (Some(crate_), None, Some(bindings)) => {
                let crate_lib = crate_.library_path(None, release)?;
                let target_dir = crate_.target_dir();
                let lib_name = crate_.library_name();
                let cpp_files = bindings.generate(&crate_lib)?;
                let cpp = CppBindingArg::with_files(&cpp_files);
                let so_file = cpp.compile_with_crate(clean, target_dir, lib_name)?;
                Ok(Some(so_file))
            }
            (_, _, _) => Ok(None),
        }
    }
}
