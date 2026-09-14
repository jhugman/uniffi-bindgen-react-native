/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::{metadata, paths, run_cmd_quietly};

/// Run a fixture test under the Wasm2 (player-based) flavor.
///
/// Builds the fixture crate once, for `wasm32-unknown-unknown` (the fixture
/// itself depends on `uniffi-runtime-wasm` via a target-specific dep), and
/// generates bindings from that same `.wasm` — `ubrn_bindgen` reads uniffi
/// metadata out of its `UNIFFI_META_*` globals, so no native build is needed.
pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();

    // Step 0: Check bootstrap
    paths::assert_wasm_bootstrap();

    let test_script = Utf8Path::new(test_script);

    // Step 1: Build the fixture crate for wasm32 in a shared target dir.
    let lib_stem = metadata::find_cdylib_name(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let shared_target_dir = Utf8PathBuf::from(target_tmpdir).join("ubrn-tests-shared/wasm2-target");
    std::fs::create_dir_all(&shared_target_dir).expect("failed to create shared target dir");
    compile_wasm32(crate_name, &shared_target_dir);
    let wasm_file = shared_target_dir
        .join("wasm32-unknown-unknown/release")
        .join(format!("{lib_stem}.wasm"));

    // Step 2: Generate TS bindings from the *unstaged* wasm. Staging strips
    // the `UNIFFI_META_*` exports and may rewrite the module, so metadata has
    // to come off the raw cargo output.
    let generated = fixture_dir.join("generated/wasm2");
    let _ = std::fs::remove_dir_all(&generated);
    let ts_dir = generated.join("ts");
    std::fs::create_dir_all(&ts_dir).expect("failed to create ts dir");
    generate_bindings(&wasm_file, &ts_dir);

    // Step 3: Stage the wasm next to the TS bindings, with DCE — fixture
    // artifacts are ours, so over-stripping fails a test rather than a user.
    ubrn_common::stage_wasm(&wasm_file, &ts_dir, &lib_stem, true)
        .unwrap_or_else(|e| panic!("staging {wasm_file}: {e:#}"));

    // Step 4: Stand in for the entrypoint a real project would use. Test
    // scripts are shared across flavors, so they import the API module
    // directly and never call `uniffiInitAsync`.
    let bootstrap = write_node_bootstrap(&ts_dir, &lib_stem);

    // Step 5: Write fixture tsconfig + run tsx.
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &fixture_dir,
        crate::Flavor::Wasm2,
    ));
    crate::run_tsx_with_preload(test_script, &bootstrap);
}

/// Write the preload that initialises the generated bindings, and return its
/// path. The index does the real loading; this only names the asset and calls
/// it, because test scripts are shared across flavors and never call it
/// themselves.
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    let path = ts_dir.join("uniffi-bootstrap.node.ts");
    std::fs::write(
        &path,
        format!(
            "import {{ uniffiInitAsync }} from \"./index.js\";\n\
             await uniffiInitAsync(new URL(\"./{lib_stem}.wasm\", import.meta.url));\n"
        ),
    )
    .unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}

/// Generate bindings via the CLI, using the `wasm2` subcommand.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("uniffi-bindgen-react-native")
            .arg("--")
            .arg("generate")
            .arg("wasm2")
            .arg("bindings")
            .arg("--library")
            .arg("--ts-dir")
            .arg(ts_dir.as_str())
            .arg(cdylib_path.as_str()),
    );
}

/// `cargo build --lib -p <crate_name> --target wasm32-unknown-unknown`.
///
/// `--lib` because only the cdylib is consumed; a fixture also declaring a
/// `[[bin]]` would otherwise build a multi-megabyte executable nobody reads.
/// No RUSTFLAGS: `ubrn_common::export_growable_table` adds the growable table
/// export to the built module instead.
fn compile_wasm32(crate_name: &str, target_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .env("CARGO_TARGET_DIR", target_dir.as_str())
            .arg("build")
            .arg("--release")
            .arg("--lib")
            .arg("-p")
            .arg(crate_name)
            .arg("--target")
            .arg("wasm32-unknown-unknown"),
    );
}
