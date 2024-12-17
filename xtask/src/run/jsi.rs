/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::{cpp_bindings::CppBindingArg, generate_bindings::render_entrypoint, RunCmd};
use crate::bootstrap::{Bootstrap, TestRunnerCmd};
use anyhow::{Ok, Result};
use camino::Utf8PathBuf;

pub(crate) struct Jsi;

impl Jsi {
    pub(crate) fn run(&self, cmd: &RunCmd) -> Result<()> {
        TestRunnerCmd.ensure_ready()?;
        let so_file = self.prepare_library_path(cmd)?;
        let js_file = cmd.js_file.prepare_for_jsi()?;
        TestRunnerCmd.run(&js_file, so_file.as_ref())?;
        Ok(())
    }

    fn prepare_library_path(&self, cmd: &RunCmd) -> Result<Option<Utf8PathBuf>> {
        let switches = &cmd.switches;
        let clean = cmd.clean;

        match (&cmd.crate_, &cmd.cpp_binding, &cmd.generate_bindings) {
            (Some(crate_), Some(cpp), _) => {
                let crate_ = crate_.cargo_build(clean)?;
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
                let profile = crate_.profile();
                let crate_ = crate_.cargo_build(clean)?;
                let crate_lib = crate_.library_path(None, profile);
                let target_dir = crate_.target_dir();
                let lib_name = crate_.library_name();
                let modules =
                    bindings.render(&crate_lib, &crate_.manifest_path().to_path_buf(), switches)?;
                let abi_dir = bindings.abi_dir();
                let entrypoint = switches.flavor.entrypoint();
                let entrypoint_cpp = abi_dir.join(entrypoint);
                render_entrypoint(switches, &entrypoint_cpp, &modules)?;

                let cpp_files: Vec<_> = modules
                    .iter()
                    .map(|m| abi_dir.join(m.cpp_filename()))
                    .chain(vec![entrypoint_cpp])
                    .collect();
                let cpp = CppBindingArg::with_files(&cpp_files);
                let so_file = cpp.compile_with_crate(clean, target_dir, lib_name)?;
                Ok(Some(so_file))
            }
            (_, _, _) => Ok(None),
        }
    }
}
