/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use ubrn_bindgen::{BindingsArgs, OutputArgs, SourceArgs};

use crate::{codegen::TurboModuleArgs, config::ProjectConfig};

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
    /// Generate the Bindings and TurboModule code from a library
    /// file and a YAML config file.
    ///
    /// This is the second step of the `--and-generate` option of the build command.
    All(GenerateAllArgs),
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
            Self::All(t) => {
                t.run()?;
                Ok(())
            }
        }
    }
}

#[derive(Args, Debug)]
pub(crate) struct GenerateAllArgs {
    /// The configuration file for this project
    #[clap(long)]
    config: Utf8PathBuf,

    /// A path to staticlib file.
    lib_file: Utf8PathBuf,
}

impl GenerateAllArgs {
    pub(crate) fn new(lib_file: Utf8PathBuf, config: Utf8PathBuf) -> Self {
        Self { lib_file, config }
    }

    pub(crate) fn run(&self) -> Result<()> {
        let project = self.project_config()?;
        let root = project.project_root();
        let pwd = ubrn_common::pwd()?;
        let lib_file = pwd.join(&self.lib_file);
        let modules = {
            let dir = project.crate_.directory()?;
            ubrn_common::cd(&dir)?;
            let ts_dir = project.bindings.ts_path(root);
            let cpp_dir = project.bindings.cpp_path(root);
            let config = project.bindings.uniffi_toml_path(root);
            if let Some(ref file) = config {
                if !file.exists() {
                    anyhow::bail!("uniffi.toml file {:?} does not exist. Either delete the uniffiToml property or supply a file", file)
                }
            }
            let bindings = BindingsArgs::new(
                SourceArgs::library(&lib_file).with_config(config),
                OutputArgs::new(&ts_dir, &cpp_dir, false),
            );

            bindings.run()?
        };
        ubrn_common::cd(&pwd)?;
        let rust_crate = project.crate_.metadata()?;
        crate::codegen::render_files(project, rust_crate, modules)?;
        Ok(())
    }

    fn project_config(&self) -> Result<ProjectConfig> {
        self.config.clone().try_into()
    }
}
