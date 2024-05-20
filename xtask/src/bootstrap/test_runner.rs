/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{fs, process::Command};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use uniffi_common::{rm_dir, run_cmd};

use crate::{
    bootstrap::HermesCmd,
    util::{build_root, repository_root},
};

use super::Bootstrap;

#[derive(Debug, Args, Default)]
pub(crate) struct TestRunnerCmd;

impl TestRunnerCmd {
    fn src_dir() -> Result<Utf8PathBuf> {
        let root = repository_root()?;
        Ok(root.join("cpp").join("test-harness"))
    }

    pub(crate) fn hermes_extension_src_dir() -> Result<Utf8PathBuf> {
        let root = repository_root()?;
        Ok(root.join("cpp/hermes-extension"))
    }

    pub(crate) fn hermes_rust_extension_src_dir() -> Result<Utf8PathBuf> {
        let root = repository_root()?;
        Ok(root.join("cpp/hermes-rust-extension"))
    }

    fn build_dir() -> Result<Utf8PathBuf> {
        let root = build_root()?;
        Ok(root.join("test-runner"))
    }

    fn exe() -> Result<Utf8PathBuf> {
        let root = Self::build_dir()?;
        Ok(root.join("test-runner"))
    }

    pub(crate) fn run(&self, js_file: &Utf8PathBuf, so_file: Option<&Utf8PathBuf>) -> Result<()> {
        let mut cmd = Command::new(Self::exe()?);
        cmd.arg(js_file);
        if let Some(so_file) = so_file {
            cmd.arg(so_file);
        }
        run_cmd(&mut cmd)?;
        Ok(())
    }
}

impl Bootstrap for TestRunnerCmd {
    fn marker() -> Result<Utf8PathBuf> {
        Self::exe()
    }

    fn clean() -> Result<()> {
        rm_dir(Self::build_dir()?)?;
        Ok(())
    }

    fn prepare(&self) -> Result<()> {
        HermesCmd::default().ensure_ready()?;
        let dir = Self::build_dir()?;
        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;

        let src_dir = TestRunnerCmd::src_dir()?;

        fs::create_dir_all(&dir)?;

        let mut cmd = Command::new("cmake");
        cmd.current_dir(&dir)
            .arg("-G")
            .arg("Ninja")
            .arg(format!("-DHERMES_SRC_DIR={}", &hermes_src))
            .arg(format!("-DHERMES_BUILD_DIR={}", hermes_build))
            .arg(&src_dir);

        run_cmd(&mut cmd)?;

        let mut cmd = Command::new("ninja");
        run_cmd(cmd.current_dir(&dir))?;

        Ok(())
    }
}
