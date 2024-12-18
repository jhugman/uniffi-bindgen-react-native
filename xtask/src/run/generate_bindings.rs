/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::fs;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_bindgen::{BindingsArgs, ModuleMetadata, OutputArgs, SourceArgs, SwitchArgs};

#[derive(Args, Debug, Clone)]
pub(crate) struct GenerateBindingsArg {
    /// Directory for the generated Typescript to put in.
    #[clap(long, requires = "abi_dir")]
    pub(crate) ts_dir: Option<Utf8PathBuf>,
    /// Directory for the generated low-level C++ or Rust to put in.
    #[clap(long, requires = "ts_dir", alias = "cpp-dir")]
    pub(crate) abi_dir: Option<Utf8PathBuf>,
    /// Optional uniffi.toml location
    #[clap(long, requires = "ts_dir")]
    pub(crate) toml: Option<Utf8PathBuf>,
}

impl GenerateBindingsArg {
    pub(crate) fn ts_dir(&self) -> Utf8PathBuf {
        self.ts_dir.clone().unwrap()
    }

    pub(crate) fn abi_dir(&self) -> Utf8PathBuf {
        self.abi_dir.clone().unwrap()
    }

    fn uniffi_toml(&self) -> Option<Utf8PathBuf> {
        self.toml.clone()
    }

    pub(crate) fn render(
        &self,
        library: &Utf8PathBuf,
        manifest_path: &Utf8PathBuf,
        switches: &SwitchArgs,
    ) -> Result<Vec<ModuleMetadata>> {
        let modules = self.render_into(
            library,
            switches,
            manifest_path,
            &self.ts_dir(),
            &self.abi_dir(),
        )?;
        Ok(modules)
    }

    pub(crate) fn render_into(
        &self,
        library: &Utf8PathBuf,
        switches: &SwitchArgs,
        manifest_path: &Utf8PathBuf,
        ts_dir: &Utf8PathBuf,
        cpp_dir: &Utf8PathBuf,
    ) -> Result<Vec<ModuleMetadata>> {
        let output = OutputArgs::new(ts_dir, cpp_dir, false);
        let toml = self.uniffi_toml().filter(|file| file.exists());
        let source = SourceArgs::library(library).with_config(toml);
        let bindings = BindingsArgs::new(switches.clone(), source, output);
        let modules = bindings.run(Some(manifest_path))?;
        Ok(modules)
    }
}

pub(crate) fn render_entrypoint(
    switches: &SwitchArgs,
    path: &Utf8Path,
    modules: &Vec<ModuleMetadata>,
) -> Result<()> {
    let string = ubrn_bindgen::generate_entrypoint(switches, modules)?;
    fs::write(path, string)?;
    Ok(())
}
