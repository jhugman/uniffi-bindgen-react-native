/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use ubrn_common::{rm_dir, run_cmd_quietly, so_extension};

use crate::{
    bootstrap::{Bootstrap, HermesCmd, TestRunnerCmd},
    util::build_root,
};

#[derive(Debug)]
pub(crate) struct CppBindingArg {
    pub(crate) cpp_files: Vec<Utf8PathBuf>,
}

impl CppBindingArg {
    pub(crate) fn with_file(cpp: Utf8PathBuf) -> Self {
        Self {
            cpp_files: vec![cpp],
        }
    }

    pub(crate) fn with_files(cpp: &[Utf8PathBuf]) -> Self {
        Self {
            cpp_files: cpp.into(),
        }
    }

    fn cpp_file(&self) -> String {
        self.cpp_files
            .iter()
            .filter_map(|f| f.canonicalize_utf8().ok())
            .map(|p| p.as_str().to_string())
            .collect::<Vec<_>>()
            .join(";")
    }

    pub(crate) fn compile_with_crate(
        &self,
        clean: bool,
        target_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<Utf8PathBuf> {
        let cpp = self.cpp_file();

        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;
        HermesCmd::default().ensure_ready()?;

        let dir = build_root()?;
        let build_dir = dir.join(lib_name).join("build-debug");
        if clean {
            rm_dir(&build_dir)?;
        }
        ubrn_common::mk_dir(&build_dir)?;

        let src_dir = TestRunnerCmd::hermes_rust_extension_src_dir()?;

        let extension_name = format!("rn-{lib_name}");

        let mut cmd = Command::new("cmake");
        run_cmd_quietly(
            cmd.current_dir(&build_dir)
                .arg("-G")
                .arg("Ninja")
                .arg(format!("-DHERMES_SRC_DIR={}", &hermes_src))
                .arg(format!("-DHERMES_BUILD_DIR={}", &hermes_build))
                .arg(format!("-DHERMES_EXTENSION_NAME={}", &extension_name))
                .arg(format!("-DRUST_LIB_NAME={lib_name}"))
                .arg(format!("-DRUST_TARGET_DIR={}/debug", &target_dir))
                .arg(format!("-DHERMES_EXTENSION_CPP={cpp}",))
                .arg(&src_dir),
        )?;

        let mut cmd = Command::new("ninja");
        run_cmd_quietly(cmd.current_dir(&build_dir))?;

        Ok(build_dir.join(format!("lib{extension_name}.{}", so_extension(None, None))))
    }

    pub(crate) fn compile_without_crate(&self, clean: bool) -> Result<Utf8PathBuf> {
        let cpp_file = self.cpp_file();
        let lib_name = self
            .cpp_files
            .first()
            .unwrap()
            .file_stem()
            .expect("filename with stem");

        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;
        HermesCmd::default().ensure_ready()?;

        let dir = build_root()?;
        let build_dir = dir.join(lib_name).join("build-debug");
        if clean {
            rm_dir(&build_dir)?;
        }
        ubrn_common::mk_dir(&build_dir)?;

        let src_dir = TestRunnerCmd::hermes_extension_src_dir()?;

        let extension_name = format!("rn-{lib_name}");

        let mut cmd = Command::new("cmake");
        run_cmd_quietly(
            cmd.current_dir(&build_dir)
                .arg("-G")
                .arg("Ninja")
                .arg(format!("-DHERMES_SRC_DIR={}", &hermes_src))
                .arg(format!("-DHERMES_BUILD_DIR={}", &hermes_build))
                .arg(format!("-DHERMES_EXTENSION_NAME={}", &extension_name))
                .arg(format!("-DHERMES_EXTENSION_CPP={cpp_file}",))
                .arg(&src_dir),
        )?;

        let mut cmd = Command::new("ninja");
        run_cmd_quietly(cmd.current_dir(&build_dir))?;

        Ok(build_dir.join(format!("lib{extension_name}.{}", so_extension(None, None))))
    }
}
