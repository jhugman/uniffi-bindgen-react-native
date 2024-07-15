/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use ubrn_bindgen::{BindingsArgs, OutputArgs, SourceArgs};

#[derive(Args, Debug, Clone)]
pub(crate) struct GenerateBindingsArg {
    /// Directory for the generated Typescript to put in.
    #[clap(long, requires = "cpp_dir")]
    pub(crate) ts_dir: Option<Utf8PathBuf>,
    /// Directory for the generated C++ to put in.
    #[clap(long, requires = "ts_dir")]
    pub(crate) cpp_dir: Option<Utf8PathBuf>,
}

impl GenerateBindingsArg {
    fn ts_dir(&self) -> Utf8PathBuf {
        self.ts_dir.clone().unwrap()
    }

    fn cpp_dir(&self) -> Utf8PathBuf {
        self.cpp_dir.clone().unwrap()
    }

    pub(crate) fn generate(&self, library: &Utf8PathBuf) -> Result<Vec<Utf8PathBuf>> {
        let output = OutputArgs::new(&self.ts_dir(), &self.cpp_dir(), false);
        let source = SourceArgs::library(library);
        let bindings = BindingsArgs::new(source, output);
        let modules = bindings.run()?;
        let cpp_dir = bindings.cpp_dir();
        Ok(modules
            .iter()
            .map(|m| cpp_dir.join(m.cpp_filename()))
            .collect())
    }
}
