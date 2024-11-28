/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use ubrn_bindgen::{BindingsArgs, OutputArgs, SourceArgs, SwitchArgs};

#[derive(Args, Debug, Clone)]
pub(crate) struct GenerateBindingsArg {
    /// Directory for the generated Typescript to put in.
    #[clap(long, requires = "cpp_dir")]
    pub(crate) ts_dir: Option<Utf8PathBuf>,
    /// Directory for the generated C++ to put in.
    #[clap(long, requires = "ts_dir", alias = "abi-dir")]
    pub(crate) cpp_dir: Option<Utf8PathBuf>,
    /// Optional uniffi.toml location
    #[clap(long, requires = "ts_dir")]
    pub(crate) toml: Option<Utf8PathBuf>,
}

impl GenerateBindingsArg {
    fn ts_dir(&self) -> Utf8PathBuf {
        self.ts_dir.clone().unwrap()
    }

    fn cpp_dir(&self) -> Utf8PathBuf {
        self.cpp_dir.clone().unwrap()
    }

    fn uniffi_toml(&self) -> Option<Utf8PathBuf> {
        self.toml.clone()
    }

    pub(crate) fn generate(
        &self,
        library: &Utf8PathBuf,
        manifest_path: &Utf8PathBuf,
        switches: &SwitchArgs,
    ) -> Result<Vec<Utf8PathBuf>> {
        let output = OutputArgs::new(&self.ts_dir(), &self.cpp_dir(), false);
        let toml = self.uniffi_toml().filter(|file| file.exists());
        let source = SourceArgs::library(library).with_config(toml);
        let bindings = BindingsArgs::new(switches.clone(), source, output);
        let modules = bindings.run(Some(manifest_path))?;
        let cpp_dir = bindings.cpp_dir();
        let index = cpp_dir.join("Entrypoint.cpp");
        bindings.render_entrypoint(&index, &modules)?;
        Ok(modules
            .iter()
            .map(|m| cpp_dir.join(m.cpp_filename()))
            .chain(vec![index])
            .collect())
    }
}
