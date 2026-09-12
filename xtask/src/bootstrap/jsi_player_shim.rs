/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use ubrn_common::{rm_dir, run_cmd};

use crate::{
    bootstrap::HermesCmd,
    util::{build_root, repository_root},
};

use super::Bootstrap;

#[derive(Debug, Args, Default)]
pub(crate) struct JsiPlayerShimCmd;

impl JsiPlayerShimCmd {
    fn src_dir() -> Result<Utf8PathBuf> {
        Ok(repository_root()?.join("runtimes").join("jsi").join("cpp"))
    }
    fn build_dir() -> Result<Utf8PathBuf> {
        Ok(build_root()?.join("jsi-player-shim"))
    }
    /// The cargo profile the player's Rust core is built with.
    ///
    /// Defaults to `debug` to keep bootstrap fast. `UBRN_PROFILE=release`
    /// builds it optimized, which matters when benchmarking: the player's FFI
    /// engine is on every call's hot path.
    fn cargo_profile() -> &'static str {
        let is_release = std::env::var("UBRN_PROFILE")
            .map(|v| v == "release")
            .unwrap_or(false);
        if is_release {
            "release"
        } else {
            "debug"
        }
    }
    fn rust_target_dir() -> Result<Utf8PathBuf> {
        Ok(repository_root()?
            .join("target")
            .join(Self::cargo_profile()))
    }
    fn include_dir() -> Result<Utf8PathBuf> {
        Ok(repository_root()?
            .join("runtimes")
            .join("jsi")
            .join("include"))
    }
    pub(crate) fn shim_lib() -> Result<Utf8PathBuf> {
        let dir = Self::build_dir()?;
        let name = if cfg!(target_os = "macos") {
            "libubrn_jsi_player.dylib"
        } else if cfg!(target_os = "windows") {
            "ubrn_jsi_player.dll"
        } else {
            "libubrn_jsi_player.so"
        };
        Ok(dir.join(name))
    }
}

impl Bootstrap for JsiPlayerShimCmd {
    fn marker() -> Result<Utf8PathBuf> {
        Self::shim_lib()
    }
    fn clean() -> Result<()> {
        rm_dir(Self::build_dir()?)?;
        Ok(())
    }
    fn prepare(&self) -> Result<()> {
        HermesCmd::default().ensure_ready()?;

        // 1. Build the Rust staticlib.
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .current_dir(repository_root()?)
            .args(["build", "-p", "uniffi-runtime-jsi"]);
        if Self::cargo_profile() == "release" {
            cargo.arg("--release");
        }
        run_cmd(&mut cargo)?;

        // 2. CMake-configure + build the shim against it.
        let dir = Self::build_dir()?;
        ubrn_common::mk_dir(&dir)?;
        let hermes_src = HermesCmd::src_dir()?;
        let hermes_build = HermesCmd::build_dir()?;

        let mut cmd = Command::new("cmake");
        cmd.current_dir(&dir)
            .arg("-G")
            .arg(if cfg!(target_os = "windows") {
                "Visual Studio 16 2019"
            } else {
                "Ninja"
            })
            .arg("-DCMAKE_BUILD_TYPE=Release")
            .arg(format!("-DHERMES_SRC_DIR={hermes_src}"))
            .arg(format!("-DHERMES_BUILD_DIR={hermes_build}"))
            .arg(format!("-DRUST_TARGET_DIR={}", Self::rust_target_dir()?))
            .arg(format!("-DUBRN_JSI_INCLUDE_DIR={}", Self::include_dir()?))
            .arg(Self::src_dir()?);
        run_cmd(&mut cmd)?;

        if cfg!(target_os = "windows") {
            let mut b = Command::new("cmake");
            run_cmd(b.current_dir(&dir).arg("--build").arg(&dir))?;
        } else {
            let mut b = Command::new("ninja");
            run_cmd(b.current_dir(&dir))?;
        }
        Ok(())
    }
}
