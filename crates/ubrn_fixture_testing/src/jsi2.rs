/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::{metadata, paths, run_cmd, run_cmd_quietly, typescript};

/// Run a fixture test under the Jsi2 (generic JSI player) flavor.
///
/// Called from proc-macro-generated `#[test]` functions.
pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();

    // Step 0: Bootstrap check (Hermes test-runner + prebuilt player shim).
    paths::assert_jsi2_bootstrap();

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");

    // Per-test output directory.
    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/{crate_name}-{test_stem}-jsi2"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    // Step 1: Build the fixture crate (produces the cdylib the player dlopens).
    crate::cargo_build(crate_name);

    // Step 2: Generate TypeScript-only bindings with the player ffi.ts. The
    // absolute cdylib path is baked into the generated getter.
    let cdylib_path = metadata::find_cdylib(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated = fixture_dir.join("generated/jsi2");
    let _ = std::fs::remove_dir_all(&generated);
    let ts_dir = generated.join("ts");
    generate_bindings(&cdylib_path, &ts_dir);

    // Step 3: Compile + bundle the TypeScript (reuse the JSI bundling path).
    let bundle = typescript::prepare_for_jsi(test_script, &out_dir, Some(&ts_dir));

    // Step 4: Run the test-runner with the prebuilt player shim as the native lib.
    let shim = paths::jsi_player_shim_lib();
    run_test_runner(&bundle, &shim);
}

/// Generate player bindings via the CLI with the jsi2 flavor.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("uniffi-bindgen-react-native")
            .arg("--")
            .arg("generate")
            .arg("jsi2")
            .arg("bindings")
            .arg("--lib-absolute")
            .arg("--library")
            .arg("--ts-dir")
            .arg(ts_dir.as_str())
            .arg(cdylib_path.as_str()),
    );
}

/// Run the test-runner binary with the JS bundle and the prebuilt player shim.
fn run_test_runner(bundle: &Utf8Path, shim: &Utf8Path) {
    let runner = paths::test_runner_binary();
    let mut cmd = Command::new(runner.as_str());
    cmd.arg(bundle.as_str()).arg(shim.as_str());
    paths::add_hermes_dll_paths(&mut cmd);
    run_cmd(&mut cmd);
}
