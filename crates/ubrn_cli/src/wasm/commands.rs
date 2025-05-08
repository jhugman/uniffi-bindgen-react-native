/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{run_cmd, CrateMetadata};

use super::config::Target;
use crate::{
    commands::building::CommonBuildArgs,
    config::{ExtraArgs, ProjectConfig},
};

#[derive(Args, Debug)]
pub(crate) struct WebBuildArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl WebBuildArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        self.cargo_build(
            &crate_.manifest_path()?,
            config.wasm.features.as_deref(),
            &crate_.crate_dir()?,
        )?;
        let metadata = crate_.metadata()?;
        let library_path = metadata.library_path(None, "debug");
        Ok(vec![library_path])
    }

    pub(crate) fn then_build(&self) -> Result<()> {
        let config = self.project_config()?;
        let project_root = config.project_root();
        let wasm_crate = {
            let manifest_path = config.wasm.manifest_path(project_root);
            let crate_dir = config.wasm.crate_dir(project_root);
            self.cargo_build_wasm(
                &manifest_path,
                &config.wasm.cargo_extras,
                &Target::Wasm32UnknownUnknown,
                &crate_dir,
            )?;
            CrateMetadata::try_from(manifest_path.to_path_buf())?
        };
        let library_path = wasm_crate.library_path(
            Some(Target::Wasm32UnknownUnknown.triple()),
            self.common_args.profile(),
        );
        self.wasm_bindgen(
            &library_path,
            &config.wasm.wasm_bindgen_extras,
            &config.bindings.ts_path(project_root).join("wasm-bindgen"),
        )?;

        Ok(())
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let config: ProjectConfig = self.config.clone().try_into()?;
        Ok(config)
    }

    fn cargo_build(
        &self,
        manifest_path: &Utf8Path,
        features: Option<&[String]>,
        rust_dir: &Utf8Path,
    ) -> Result<()> {
        println!("Compiling for wasm32 manifest at {manifest_path}");
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--manifest-path")
            .arg(manifest_path)
            .current_dir(rust_dir);
        if let Some(features) = features {
            cmd.arg("--features").arg(features.join(","));
        }
        run_cmd(&mut cmd)?;
        Ok(())
    }

    fn cargo_build_wasm(
        &self,
        manifest_path: &Utf8Path,
        cargo_extras: &ExtraArgs,
        target: &Target,
        rust_dir: &Utf8Path,
    ) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--target")
            .arg(target.triple());
        let profile = self.common_args.profile();
        if profile != "debug" {
            cmd.arg("--profile").arg(profile);
        }
        cmd.args(cargo_extras.clone()).current_dir(rust_dir);
        run_cmd(&mut cmd)?;
        Ok(())
    }

    fn wasm_bindgen(
        &self,
        library_path: &Utf8Path,
        wasm_bindgen_extras: &ExtraArgs,
        out_dir: &Utf8Path,
    ) -> Result<()> {
        let mut cmd = Command::new("wasm-bindgen");
        cmd.arg("--target")
            .arg("bundler")
            .arg("--omit-default-module-path")
            .arg("--out-name")
            .arg("index")
            .arg("--out-dir")
            .arg(out_dir)
            .args(wasm_bindgen_extras.clone())
            .arg(library_path);
        run_cmd(&mut cmd)?;
        Ok(())
    }
}
