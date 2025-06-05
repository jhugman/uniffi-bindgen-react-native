/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::convert::TryFrom;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{self, Args, Subcommand};

use ubrn_bindgen::{AbiFlavor, BindingsArgs, ModuleMetadata, SwitchArgs};

#[cfg(feature = "wasm")]
use crate::wasm;
use crate::{
    codegen::{files, get_template_config, render_files},
    config::ProjectConfig,
    jsi, Platform,
};

use super::ConfigArgs;

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
    /// Commands which re-direct to the jsi version.
    ///
    /// These are now deprecated and so hidden.
    #[clap(hide = true)]
    Bindings(BindingsArgs),
    #[clap(hide = true)]
    TurboModule(jsi::TurboModuleArgs),
    #[clap(hide = true)]
    All(GenerateAllArgs),

    /// Commands to generate the JSI bindings and turbo-module code.
    #[clap(aliases = ["react-native", "rn"])]
    Jsi(jsi::CmdArg),

    /// Commands to generate a WASM crate.
    #[cfg(feature = "wasm")]
    #[clap(aliases = ["web"])]
    Wasm(wasm::CmdArg),
}

impl GenerateCmd {
    pub(crate) fn run(&self) -> Result<()> {
        match self {
            Self::Bindings(b) => {
                b.run(None)?;
                Ok(())
            }
            Self::TurboModule(t) => {
                t.run()?;
                Ok(())
            }
            Self::All(t) => {
                let t = GenerateAllCommand::try_from(t)?;
                t.run()?;
                Ok(())
            }
            Self::Jsi(jsi) => {
                jsi.run()?;
                Ok(())
            }
            #[cfg(feature = "wasm")]
            Self::Wasm(wasm) => {
                wasm.run()?;
                Ok(())
            }
        }
    }
}

#[derive(Args, Debug)]
pub(crate) struct GenerateAllArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    #[cfg(feature = "wasm")]
    #[command(flatten)]
    switches: SwitchArgs,

    /// A path to staticlib file.
    lib_file: Utf8PathBuf,
}

#[derive(Debug)]
pub(crate) struct GenerateAllCommand {
    /// The configuration file for this project
    project_config: ProjectConfig,

    /// A path to staticlib file.
    lib_file: Utf8PathBuf,

    platform: Option<Platform>,
}

impl GenerateAllCommand {
    pub(crate) fn new(lib_file: Utf8PathBuf, project_config: ProjectConfig) -> Self {
        Self {
            lib_file,
            project_config,
            platform: None,
        }
    }

    pub(crate) fn platform_specific(
        lib_file: Utf8PathBuf,
        project_config: ProjectConfig,
        platform: Platform,
    ) -> Self {
        Self {
            lib_file,
            project_config,
            platform: Some(platform),
        }
    }

    fn switches(&self) -> SwitchArgs {
        let flavor = self.platform.as_ref().map_or(AbiFlavor::Jsi, |p| p.into());
        SwitchArgs { flavor }
    }

    pub(crate) fn run(&self) -> Result<()> {
        let pwd = ubrn_common::pwd()?;
        let lib_file = pwd.join(&self.lib_file);

        // Step 1: Generate bindings
        let modules = self.generate_bindings(&lib_file)?;

        // Step 2: Generate template files
        self.generate_template_files(modules)?;

        Ok(())
    }

    fn generate_bindings(&self, lib_file: &Utf8PathBuf) -> Result<Vec<ModuleMetadata>> {
        let project = &self.project_config;
        let switches = self.switches();
        let pwd = ubrn_common::pwd()?;
        let bindings = self.create_bindings_command(lib_file, project, switches)?;

        ubrn_common::cd(&project.crate_.crate_dir()?)?;
        let manifest_path = project.crate_.manifest_path()?;
        let modules = bindings.run(Some(&manifest_path))?;
        ubrn_common::cd(&pwd)?;

        Ok(modules)
    }

    fn create_bindings_command(
        &self,
        lib_file: &Utf8PathBuf,
        project: &ProjectConfig,
        switches: SwitchArgs,
    ) -> Result<BindingsArgs, anyhow::Error> {
        Ok(match self.platform {
            #[cfg(feature = "wasm")]
            Some(Platform::Wasm) => wasm::bindings(project, switches, lib_file)?,
            _ => jsi::bindings(project, switches, lib_file)?,
        })
    }

    fn generate_template_files(&self, modules: Vec<ModuleMetadata>) -> Result<()> {
        let project = &self.project_config;
        let rust_crate = project.crate_.metadata()?;
        let config = get_template_config(project.clone(), rust_crate, modules);

        let files = match &self.platform {
            Some(platform) => files::get_files_for(config.clone(), platform),
            None => files::get_files(config.clone()),
        };

        render_files(config, files.into_iter())
    }
}

impl TryFrom<&GenerateAllArgs> for GenerateAllCommand {
    type Error = anyhow::Error;

    fn try_from(value: &GenerateAllArgs) -> Result<Self> {
        let project_config = value.config.clone().try_into()?;
        Ok(GenerateAllCommand::new(
            value.lib_file.clone(),
            project_config,
        ))
    }
}
