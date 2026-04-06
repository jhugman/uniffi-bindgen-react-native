/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;
use std::sync::Once;

use camino::Utf8Path;

use crate::{metadata, paths, run_cmd_quietly};

static BUILD_NAPI_RUNTIME: Once = Once::new();

/// Run a fixture test under the Napi (Node.js player) flavor.
///
/// Called from proc-macro-generated `#[test]` functions.
pub fn run_test(crate_name: &str, test_script: &str, _target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();

    // Step 0: Ensure napi runtime is built, then check bootstrap.
    ensure_napi_runtime_built();
    paths::assert_napi_bootstrap();

    let test_script = Utf8Path::new(test_script);

    // Step 1: Build the fixture crate.
    crate::cargo_build(crate_name);

    // Step 2: Generate bindings via CLI with napi flavor.
    let cdylib_path = metadata::find_cdylib(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated_napi = fixture_dir.join("generated/napi");
    let _ = std::fs::remove_dir_all(&generated_napi);
    let ts_dir = generated_napi.join("ts");
    generate_bindings(&cdylib_path, &ts_dir);

    // Step 3: Write fixture tsconfig, then run tsx with the library path.
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &fixture_dir,
        crate::Flavor::Napi,
    ));
    run_tsx_with_lib(test_script, &cdylib_path);
}

/// Build the N-API runtime (debug mode) if not already built.
fn ensure_napi_runtime_built() {
    BUILD_NAPI_RUNTIME.call_once(|| {
        let napi_dir = paths::napi_runtime_dir();
        run_cmd_quietly(
            Command::new("npm")
                .arg("run")
                .arg("build:debug")
                .current_dir(napi_dir.as_str()),
        );
    });
}

/// Generate bindings via the CLI with --flavor napi.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("uniffi-bindgen-react-native")
            .arg("--")
            .arg("generate")
            .arg("napi")
            .arg("bindings")
            .arg("--library")
            .arg("--ts-dir")
            .arg(ts_dir.as_str())
            .arg(cdylib_path.as_str()),
    );
}

/// Run tsx with UNIFFI_LIB_PATH set so the generated code can find the library.
fn run_tsx_with_lib(test_script: &Utf8Path, cdylib_path: &Utf8Path) {
    let tsx = paths::node_modules_bin().join("tsx");
    run_cmd_quietly(
        Command::new(tsx.as_str())
            .env("UNIFFI_LIB_PATH", cdylib_path.as_str())
            .arg(test_script.as_str()),
    );
}
