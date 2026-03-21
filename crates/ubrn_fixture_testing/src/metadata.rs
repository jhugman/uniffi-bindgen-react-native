/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::{Utf8Path, Utf8PathBuf};

/// Derive the workspace target directory from `CARGO_TARGET_TMPDIR`.
///
/// `CARGO_TARGET_TMPDIR` = `<target_dir>/tmp`, so we just strip the last
/// component. This avoids the ~130ms `cargo metadata` overhead per process.
pub(crate) fn target_dir_from_tmpdir(target_tmpdir: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(target_tmpdir)
        .parent()
        .expect("target_tmpdir has no parent")
        .to_path_buf()
}

/// Derive the fixture directory from the test script path.
///
/// The test script is `<CARGO_MANIFEST_DIR>/tests/bindings/test_foo.ts`,
/// so we walk up until we find a `Cargo.toml`.
pub(crate) fn fixture_dir_from_script(test_script: &Utf8Path) -> Utf8PathBuf {
    let mut dir = test_script
        .parent()
        .expect("test_script has no parent directory");
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir.to_path_buf();
        }
        dir = dir
            .parent()
            .unwrap_or_else(|| panic!("Cargo.toml not found above {test_script}"));
    }
}

/// Read the cdylib target name from a fixture's Cargo.toml.
///
/// Parses the `[lib] name = "..."` field. Falls back to the package name
/// (with hyphens replaced by underscores) if no explicit lib name is set.
pub(crate) fn read_cdylib_name(fixture_dir: &Utf8Path) -> String {
    let cargo_toml_path = fixture_dir.join("Cargo.toml");
    let contents = std::fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("failed to read {cargo_toml_path}: {e}"));

    // Look for [lib] section and extract `name = "..."`
    let mut in_lib_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_lib_section = trimmed == "[lib]";
            continue;
        }
        if in_lib_section {
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    let name = rest.trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
    }

    // Fallback: derive from package name
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let name = rest.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return name.replace('-', "_");
                }
            }
        }
    }

    panic!("could not determine cdylib name from {cargo_toml_path}");
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

/// Find the cdylib artifact path given a library name and target directory.
///
/// Checks `target/debug/` first (produced by `cargo build`), then
/// `target/debug/deps/` (produced by `cargo test`).
pub(crate) fn find_cdylib_path(lib_name: &str, target_dir: &Utf8Path) -> Utf8PathBuf {
    let filename = format!("lib{lib_name}.{}", shared_lib_ext());
    let primary = target_dir.join("debug").join(&filename);
    if primary.exists() {
        return primary;
    }
    let deps = target_dir.join("debug").join("deps").join(&filename);
    if deps.exists() {
        return deps;
    }
    // Return primary path — will error downstream with a clear message
    primary
}
