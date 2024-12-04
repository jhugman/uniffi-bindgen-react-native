/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{self, Args, Subcommand};
use std::convert::TryFrom;
use ubrn_bindgen::{AbiFlavor, BindingsArgs, OutputArgs, SourceArgs, SwitchArgs};

use crate::{codegen::TurboModuleArgs, config::ProjectConfig, Platform};

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
    /// Generate just the Typescript and C++ bindings
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
        }
    }
}

#[derive(Args, Debug)]
pub(crate) struct GenerateAllArgs {
    /// The configuration file for this project
    #[clap(long)]
    config: Utf8PathBuf,

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
        let project = self.project_config()?;
        let root = project.project_root();
        let pwd = ubrn_common::pwd()?;
        let lib_file = pwd.join(&self.lib_file);
        let switches = self.switches();
        let modules = {
            ubrn_common::cd(&project.crate_.crate_dir()?)?;
            let ts_dir = project.bindings.ts_path(root);
            let cpp_dir = project.bindings.cpp_path(root);
            let config = project.bindings.uniffi_toml_path(root);
            if let Some(ref file) = config {
                if !file.exists() {
                    anyhow::bail!("uniffi.toml file {:?} does not exist. Either delete the uniffiToml property or supply a file", file)
                }
            }
            let manifest_path = project.crate_.manifest_path()?;
            let bindings = BindingsArgs::new(
                switches.clone(),
                SourceArgs::library(&lib_file).with_config(config),
                OutputArgs::new(&ts_dir, &cpp_dir, false),
            );

            bindings.run(Some(&manifest_path))?
        };
        ubrn_common::cd(&pwd)?;
        let rust_crate = project.crate_.metadata()?;
        crate::codegen::render_files(self.platform.clone(), project, rust_crate, modules)?;
        Ok(())
    }

    fn project_config(&self) -> Result<ProjectConfig> {
        Ok(self.project_config.clone())
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
