/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::{Utf8Path, Utf8PathBuf};
use std::sync::LazyLock;

/// Repository root, derived from target directory.
///
/// `target_dir` is typically `<repo_root>/target`, so repo_root is its parent.
/// For custom CARGO_TARGET_DIR locations this falls back to walking up from
/// the target dir to find the workspace Cargo.toml.
pub(crate) fn repo_root() -> &'static Utf8Path {
    static ROOT: LazyLock<Utf8PathBuf> = LazyLock::new(|| {
        let target = crate::target_dir();
        // Common case: target dir is <repo_root>/target
        if let Some(parent) = target.parent() {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
        }
        // Fallback: walk up from target dir
        let mut dir = target;
        loop {
            if dir.join("Cargo.toml").exists() {
                return dir.to_path_buf();
            }
            dir = dir
                .parent()
                .expect("workspace Cargo.toml not found above target dir");
        }
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
    build_root().join("test-runner").join("test-runner")
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

fn assert_node_modules() {
    let nm = repo_root().join("node_modules");
    assert!(
        nm.exists(),
        "node_modules not found at {nm}. Run `cargo xtask bootstrap` first."
    );
}
