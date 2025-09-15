/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::{Context, Ok, Result};
use camino::{Utf8Path, Utf8PathBuf};
use ubrn_common::{run_cmd_quietly, CrateMetadata};

use super::{generate_bindings::render_entrypoint, NodeJs, RunCmd};
use crate::{
    bootstrap::{Bootstrap, YarnCmd},
    util::repository_root,
};

pub(crate) struct Wasm;

impl Wasm {
    pub(crate) fn run(&self, cmd: &RunCmd) -> Result<()> {
        YarnCmd.ensure_ready()?;
        let _so_file = self.prepare_library_path(cmd)?;
        let js_file = cmd
            .js_file
            .file
            .canonicalize_utf8()
            .context(format!("{} expected, but wasn't there", &cmd.js_file.file))?;
        NodeJs.tsx(&js_file)?;
        Ok(())
    }

    fn prepare_library_path(&self, cmd: &RunCmd) -> Result<Option<Utf8PathBuf>> {
        let switches = &cmd.switches;
        let clean = cmd.clean;

        match (&cmd.crate_, &cmd.cpp_binding, &cmd.generate_bindings) {
            //         (Some(crate_), Some(cpp), _) => {
            //             let crate_ = crate_.cargo_build(clean)?;
            //             let target_dir = crate_.target_dir();
            //             let lib_name = crate_.library_name();
            //             let cpp = CppBindingArg::with_file(cpp.clone());
            //             let so_file = cpp.compile_with_crate(clean, target_dir, lib_name)?;
            //             Ok(Some(so_file))
            //         }
            //         (None, Some(cpp), _) => {
            //             let cpp = CppBindingArg::with_file(cpp.clone());
            //             let so_file = cpp.compile_without_crate(clean)?;
            //             Ok(Some(so_file))
            //         }
            (Some(crate_), None, Some(bindings)) => {
                let profile = crate_.profile();
                let crate_ = crate_.cargo_build(clean)?;
                let crate_lib = crate_.library_path(None, profile, None);

                let generated_crate = bindings.abi_dir();
                let src_dir = generated_crate.join("src");
                ubrn_common::mk_dir(&src_dir)?;

                let modules = bindings.render_into(
                    &crate_lib,
                    switches,
                    &crate_.manifest_path().to_path_buf(),
                    &bindings.ts_dir(),
                    &src_dir,
                )?;

                let lib_rs = generated_crate.join(switches.flavor.entrypoint());
                render_entrypoint(switches, &lib_rs, &crate_, &modules)?;

                let cargo_toml =
                    self.render_cargo_toml(&generated_crate.canonicalize_utf8()?, &crate_)?;

                self.compile_wasm32_unknown_unknown(&cargo_toml)?;

                let wasm_file = self.find_wasm_file(&generated_crate)?;

                self.run_wasm_bindgen(&wasm_file, &bindings.ts_dir().join("wasm-bindgen"))?;
                Ok(None)
            }
            (_, _, _) => Ok(None),
        }
    }

    fn render_cargo_toml(
        &self,
        generated_crate: &Utf8Path,
        crate_under_test: &CrateMetadata,
    ) -> Result<Utf8PathBuf> {
        let src = include_str!("Cargo.template.toml");
        let cargo_toml = generated_crate.join("Cargo.toml");

        let uniffi_runtime_javascript = repository_root()?.join("crates/uniffi-runtime-javascript");
        let cargo_toml_src = src
            .replace("{{crate_name}}", crate_under_test.package_name())
            .replace(
                "{{crate_path}}",
                pathdiff::diff_utf8_paths(crate_under_test.crate_dir(), generated_crate)
                    .expect("Should be able to find a relative path")
                    .as_str(),
            )
            .replace(
                "{{uniffi_runtime_javascript}}",
                pathdiff::diff_utf8_paths(uniffi_runtime_javascript, generated_crate)
                    .expect("Should be able to find a relative path")
                    .as_str(),
            );

        ubrn_common::write_file(&cargo_toml, cargo_toml_src)?;
        Ok(cargo_toml)
    }

    fn compile_wasm32_unknown_unknown(&self, cargo_toml: &Utf8Path) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .args(["--target", "wasm32-unknown-unknown"])
            .args(["--manifest-path", cargo_toml.as_str()]);

        run_cmd_quietly(&mut cmd)?;
        Ok(())
    }

    fn find_wasm_file(&self, generated_crate: &Utf8Path) -> Result<Utf8PathBuf> {
        Ok(generated_crate.join("target/wasm32-unknown-unknown/debug/my_test_crate.wasm"))
    }

    fn run_wasm_bindgen(&self, wasm_file: &Utf8Path, out_dir: &Utf8Path) -> Result<()> {
        let mut cmd = Command::new("wasm-bindgen");
        cmd.args(["--target", "bundler"])
            .arg("--omit-default-module-path")
            .args(["--out-name", "index"])
            .args(["--out-dir", out_dir.as_str()])
            .arg(wasm_file.as_str());

        run_cmd_quietly(&mut cmd)?;
        Ok(())
    }
}
