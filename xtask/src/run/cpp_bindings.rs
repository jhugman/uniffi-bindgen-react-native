use std::{fs, process::Command};

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use uniffi_common::{rm_dir, run_cmd_quietly};

use crate::{
    bootstrap::{Bootstrap, HermesCmd, TestRunnerCmd},
    util::{build_root, so_extension_name},
};

#[derive(Debug, Args)]
pub(crate) struct CppBindingArg {
    #[clap(long)]
    pub(crate) cpp: Option<Utf8PathBuf>,
}

impl CppBindingArg {
    pub(crate) fn new(cpp: Utf8PathBuf) -> Self {
        Self { cpp: Some(cpp) }
    }

    pub(crate) fn cpp_file(&self) -> &Utf8Path {
        self.cpp.as_ref().expect("CPP not specified")
    }

    pub(crate) fn compile_with_crate(
        &self,
        clean: bool,
        target_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<Utf8PathBuf> {
        let cpp_file = self.cpp_file();

        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;
        HermesCmd::default().ensure_ready()?;

        let dir = build_root()?;
        let build_dir = dir.join(lib_name).join("build-debug");
        if clean {
            rm_dir(&build_dir)?;
        }
        fs::create_dir_all(&build_dir)?;

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
                .arg(format!("-DRUST_TARGET_DIR={}", &target_dir))
                .arg(format!("-DHERMES_EXTENSION_CPP={cpp_file}",))
                .arg(&src_dir),
        )?;

        let mut cmd = Command::new("ninja");
        run_cmd_quietly(cmd.current_dir(&build_dir))?;

        Ok(build_dir.join(format!("lib{extension_name}.{}", so_extension_name())))
    }

    pub(crate) fn compile_without_crate(&self, clean: bool) -> Result<Utf8PathBuf> {
        let cpp_file = self.cpp_file();
        let lib_name = cpp_file.file_stem().expect("filename with stem");

        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;
        HermesCmd::default().ensure_ready()?;

        let dir = build_root()?;
        let build_dir = dir.join(lib_name).join("build-debug");
        if clean {
            rm_dir(&build_dir)?;
        }
        fs::create_dir_all(&build_dir)?;

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

        Ok(build_dir.join(format!("lib{extension_name}.{}", so_extension_name())))
    }
}
