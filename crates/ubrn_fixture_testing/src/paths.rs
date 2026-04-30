/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use crate::metadata;

/// Repository root, derived from workspace metadata.
pub(crate) fn repo_root() -> &'static Utf8Path {
    static ROOT: LazyLock<Utf8PathBuf> = LazyLock::new(|| {
        let meta = metadata::workspace_metadata();
        meta.workspace_root.clone()
    });
    &ROOT
}

pub(crate) fn build_root() -> Utf8PathBuf {
    repo_root().join("build")
}

pub(crate) fn node_modules_bin() -> Utf8PathBuf {
    repo_root().join("node_modules").join(".bin")
}

pub(crate) fn hermes_src_dir() -> Utf8PathBuf {
    repo_root().join("cpp_modules").join("hermes")
}

pub(crate) fn hermes_build_dir() -> Utf8PathBuf {
    build_root().join("hermes")
}

pub(crate) fn test_runner_binary() -> Utf8PathBuf {
    let dir = build_root().join("test-runner");
    if cfg!(target_os = "windows") {
        dir.join("Debug").join("test-runner.exe")
    } else {
        dir.join("test-runner")
    }
}

/// Panics with a helpful message if required bootstrap artifacts are missing.
pub(crate) fn assert_jsi_bootstrap() {
    let runner = test_runner_binary();
    assert!(
        runner.exists(),
        "Hermes test-runner not found at {runner}. Run `cargo xtask bootstrap` first."
    );
    let hermes = hermes_build_dir();
    assert!(
        hermes.exists(),
        "Hermes build not found at {hermes}. Run `cargo xtask bootstrap` first."
    );
    assert_node_modules();
}

pub(crate) fn assert_wasm_bootstrap() {
    assert_node_modules();
}

<<<<<<< fix-windows-tests
/// On Windows, DLLs must be on PATH at runtime. This is not an issue on Linux/macOS as the
/// binary's rpath tells the linker where to find the shared libraries.
/// This adds the Hermes DLL directory to PATH on the given command.
pub(crate) fn add_hermes_dll_paths(cmd: &mut Command) {
    if cfg!(target_os = "windows") {
        let hermes_dll_dir = hermes_build_dir().join("API/hermes/Debug");
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", hermes_dll_dir, path));
=======
/// Path to the napi runtime directory within the repo.
pub(crate) fn napi_runtime_dir() -> Utf8PathBuf {
    repo_root().join("runtimes").join("napi")
}

pub(crate) fn assert_napi_bootstrap() {
    assert_node_modules();
    let napi_dir = napi_runtime_dir();
    let node_file = napi_dir.join(format!(
        "uniffi-runtime-napi.{}.node",
        napi_platform_triple()
    ));
    assert!(
        node_file.exists(),
        "N-API runtime not found at {node_file}. Run `cd runtimes/napi && npm run build:debug` first."
    );
}

fn napi_platform_triple() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "darwin-arm64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "darwin-x64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-arm64-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64-gnu"
    } else {
        panic!("Unsupported platform for N-API tests")
>>>>>>> main
    }
}

fn assert_node_modules() {
    let nm = repo_root().join("node_modules");
    assert!(
        nm.exists(),
        "node_modules not found at {nm}. Run `cargo xtask bootstrap` first."
    );
}
