/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{run_cmd, run_in_process, CrateMetadata};

use super::{
    config::{Target, WasmTarget},
    WasmConfig,
};
use crate::{
    commands::{building::CommonBuildArgs, ConfigArgs},
    config::{ExtraArgs, ProjectConfig},
};

#[derive(Args, Debug)]
pub(crate) struct WebBuildArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    /// Opts out of generating the bindings and wasm-crate.
    #[clap(long, conflicts_with_all = ["and_generate"])]
    pub(crate) no_generate: bool,

    /// Opts out of generating running wasm-pack on the generated wasm-crate.
    #[clap(long, conflicts_with_all = ["no_generate"])]
    pub(crate) no_wasm_pack: bool,

    /// Target passed to wasm-pack/wasm-bindgen.
    ///
    /// Overrides the setting in the config file.
    ///
    /// If that is missing, then default to "web".
    #[clap(long, conflicts_with_all = ["no_generate", "no_wasm_pack"])]
    target: Option<WasmTarget>,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl WebBuildArgs {
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        self.cargo_build(&crate_.manifest_path()?, &config.wasm, &crate_.crate_dir()?)?;
        let metadata = crate_.metadata()?;
        let library_path = metadata.library_path(None, "debug", None);
        Ok(vec![library_path])
    }

    pub(crate) fn then_build(&self) -> Result<()> {
        let config = self.project_config()?;
        let target = config.wasm.targets.first().cloned().unwrap_or_default();
        let project_root = config.project_root();
        let wasm_crate = {
            let manifest_path = config.wasm.manifest_path(project_root);
            let crate_dir = config.wasm.crate_dir(project_root);
            self.cargo_build_wasm(
                &manifest_path,
                &config.wasm.cargo_extras,
                &target,
                &crate_dir,
                &config.wasm.rustflags,
            )?;
            CrateMetadata::try_from(manifest_path.to_path_buf())?
        };
        let library_path =
            wasm_crate.library_path(Some(target.triple()), self.common_args.profile(), None);
        let target = self
            .target
            .clone()
            .unwrap_or_else(|| config.wasm.target.clone());
        self.wasm_bindgen(
            &library_path,
            &target,
            &config.wasm.wasm_bindgen_extras,
            &config
                .wasm_bindings_ts_path(project_root)
                .join("wasm-bindgen"),
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
        wasm_config: &WasmConfig,
        rust_dir: &Utf8Path,
    ) -> Result<()> {
        println!("Compiling for wasm32 manifest at {manifest_path}");
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--manifest-path")
            .arg(manifest_path)
            .current_dir(rust_dir);
        if let Some(features) = &wasm_config.features {
            cmd.arg("--features").arg(features.join(","));
        }
        if let Some(default_features) = &wasm_config.default_features {
            if *default_features {
                cmd.arg("--default-features");
            } else {
                cmd.arg("--no-default-features");
            }
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
        rustflags: &ExtraArgs,
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

        // Apply RUSTFLAGS if specified
        let rustflags_vec: Vec<String> = rustflags.clone().into_iter().collect();
        if !rustflags_vec.is_empty() {
            let rustflags_str = rustflags_vec.join(" ");
            cmd.env("RUSTFLAGS", rustflags_str);
        }

        run_cmd(&mut cmd)?;
        Ok(())
    }

    /// Run the wasm-bindgen rewriter on `library_path`. Uses the
    /// `wasm-bindgen-cli-support` library directly rather than shelling
    /// out, so the version follows the workspace's pin and clients don't
    /// need a separately-installed `wasm-bindgen` binary.
    ///
    /// `wasm_bindgen_extras` is the project-config escape hatch for raw CLI
    /// args. Common flags such as `--debug` and `--keep-debug` map onto
    /// `Bindgen` methods; anything unrecognized errors out with a migration
    /// message rather than being dropped silently.
    fn wasm_bindgen(
        &self,
        library_path: &Utf8Path,
        target: &WasmTarget,
        wasm_bindgen_extras: &ExtraArgs,
        out_dir: &Utf8Path,
    ) -> Result<()> {
        use wasm_bindgen_cli_support::Bindgen;
        let mut bindgen = Bindgen::new();
        bindgen.input_path(library_path.as_std_path());
        bindgen.out_name("index");
        bindgen.omit_default_module_path(true);
        match target {
            WasmTarget::Bundler => bindgen.bundler(true)?,
            WasmTarget::Nodejs => bindgen.nodejs(true)?,
            WasmTarget::Web => bindgen.web(true)?,
            WasmTarget::NoModules => bindgen.no_modules(true)?,
            WasmTarget::Deno => bindgen.deno(true)?,
            WasmTarget::ExperimentalNodejsModule => bindgen.nodejs_module(true)?,
        };
        for extra in wasm_bindgen_extras.clone() {
            match extra.as_str() {
                "--debug" => {
                    bindgen.debug(true);
                }
                "--keep-debug" => {
                    bindgen.keep_debug(true);
                }
                "--keep-lld-exports" => {
                    bindgen.keep_lld_exports(true);
                }
                "--no-demangle" => {
                    bindgen.demangle(false);
                }
                "--remove-name-section" => {
                    bindgen.remove_name_section(true);
                }
                "--remove-producers-section" => {
                    bindgen.remove_producers_section(true);
                }
                "--no-typescript" => {
                    bindgen.typescript(false);
                }
                "--reference-types" => {
                    // wasm-bindgen 0.2.114 deprecates this — reference types
                    // are auto-detected from `-Ctarget-feature=+reference-types`.
                    // Drop silently rather than break old configs.
                }
                "--split-linked-modules" => {
                    bindgen.split_linked_modules(true);
                }
                other => anyhow::bail!(
                    "wasm_bindgen_extras: {other} not yet mapped to wasm-bindgen-cli-support; \
                     either drop it or add a `Bindgen` method match in src/wasm/commands.rs"
                ),
            }
        }
        // The equivalent `wasm-bindgen` command line, for the benefit of
        // anything watching what the build does from the outside.
        let mut descriptor = Command::new("wasm-bindgen");
        descriptor
            .arg("--target")
            .arg(target.to_string())
            .arg("--omit-default-module-path")
            .arg("--out-name")
            .arg("index")
            .arg("--out-dir")
            .arg(out_dir)
            .args(wasm_bindgen_extras.clone())
            .arg(library_path);

        run_in_process(&descriptor, || {
            bindgen.generate(out_dir.as_std_path())?;
            Ok(())
        })
    }
}
