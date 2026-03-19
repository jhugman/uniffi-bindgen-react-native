/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, MetadataCommand};
use std::sync::LazyLock;

static METADATA: LazyLock<Metadata> = LazyLock::new(|| {
    MetadataCommand::new()
        .exec()
        .expect("failed to run cargo metadata")
});

pub(crate) fn workspace_metadata() -> &'static Metadata {
    &METADATA
}

fn find_package(crate_name: &str) -> &'static cargo_metadata::Package {
    let meta = workspace_metadata();
    meta.packages
        .iter()
        .find(|p| p.name == crate_name)
        .unwrap_or_else(|| panic!("package {crate_name} not found in workspace"))
}

/// Find a package in the workspace by name and return its manifest directory.
pub(crate) fn find_package_dir(crate_name: &str) -> Utf8PathBuf {
    find_package(crate_name)
        .manifest_path
        .parent()
        .expect("manifest_path has no parent")
        .to_path_buf()
}

/// Find the cdylib target name for a package (e.g. "uniffi_arithmetic").
pub(crate) fn find_cdylib_name(crate_name: &str) -> String {
    let pkg = find_package(crate_name);
    let lib_target = pkg
        .targets
        .iter()
        .find(|t| t.is_cdylib())
        .unwrap_or_else(|| panic!("no cdylib target in {crate_name}"));
    lib_target.name.clone()
}

/// Platform-specific shared library extension.
pub(crate) fn shared_lib_ext() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

/// Find the cdylib artifact for a package.
pub(crate) fn find_cdylib(crate_name: &str) -> Utf8PathBuf {
    let lib_name = find_cdylib_name(crate_name);
    find_cdylib_from_name(&lib_name)
}

/// Find the cdylib artifact given a library name.
pub(crate) fn find_cdylib_from_name(lib_name: &str) -> Utf8PathBuf {
    let target_dir = &workspace_metadata().target_directory;
    target_dir
        .join("debug")
        .join(format!("lib{lib_name}.{}", shared_lib_ext()))
}
