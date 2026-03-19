/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use pathdiff::diff_utf8_paths;

use crate::{metadata, paths, run_cmd_quietly};

/// The Cargo.toml template for generated WASM crates.
const CARGO_TEMPLATE: &str = include_str!("Cargo.template.toml");

/// Run a fixture test under the WASM flavor.
///
/// Called from proc-macro-generated `#[test]` functions.
pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();

    // Step 0: Check bootstrap
    paths::assert_wasm_bootstrap();

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");

    // Per-test output directory
    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/{crate_name}-{test_stem}-wasm"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    // Step 1: Build the fixture crate (native, for metadata extraction)
    crate::cargo_build(crate_name);

    // Step 2: Generate bindings via CLI
    // All generated artifacts go under fixture's generated/wasm/ for easy inspection.
    // Test scripts use @generated/* imports which are resolved via tsconfig.
    let cdylib_path = metadata::find_cdylib(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated_wasm = fixture_dir.join("generated/wasm");
    let _ = std::fs::remove_dir_all(&generated_wasm);
    let ts_dir = generated_wasm.join("ts");
    let wasm_crate_src = generated_wasm.join("rs");
    std::fs::create_dir_all(&wasm_crate_src).expect("failed to create generated/wasm/rs dir");
    let wasm_crate_dir = out_dir.join("wasm-crate");
    generate_bindings(&cdylib_path, &ts_dir, &wasm_crate_src);

    // Step 3: Generate lib.rs entrypoint
    let lib_name = metadata::find_cdylib_name(crate_name);
    generate_lib_rs(&wasm_crate_src, &lib_name);

    // Step 4: Generate Cargo.toml from template
    std::fs::create_dir_all(&wasm_crate_dir).expect("failed to create wasm-crate dir");
    let canonical_wasm_crate = Utf8PathBuf::try_from(
        std::path::Path::new(wasm_crate_dir.as_str())
            .canonicalize()
            .expect("failed to canonicalize wasm-crate dir"),
    )
    .expect("non-UTF-8 path");
    generate_cargo_toml(
        &canonical_wasm_crate,
        crate_name,
        &fixture_dir,
        &wasm_crate_src,
    );

    // Step 5: Build for wasm32-unknown-unknown
    let cargo_toml = wasm_crate_dir.join("Cargo.toml");
    compile_wasm32(&cargo_toml);

    // Step 6: Run wasm-bindgen
    // Output goes next to the TypeScript bindings so imports resolve correctly.
    let wasm_file = wasm_crate_dir.join("target/wasm32-unknown-unknown/debug/my_test_crate.wasm");
    let wasm_bindgen_dir = ts_dir.join("wasm-bindgen");
    std::fs::create_dir_all(&wasm_bindgen_dir).expect("failed to create wasm-bindgen dir");
    run_wasm_bindgen(&wasm_file, &wasm_bindgen_dir);

    // Step 7: Write fixture tsconfig for @generated/* resolution, then run tsx.
    // The guard ensures cleanup even if run_tsx panics.
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &fixture_dir,
        crate::Flavor::Wasm,
    ));
    crate::run_tsx(test_script);
}

/// Generate bindings via the CLI.
///
/// For WASM, the `--cpp-dir` flag points to where Rust WASM binding `.rs` files go.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path, rs_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("uniffi-bindgen-react-native")
            .arg("--")
            .arg("generate")
            .arg("wasm")
            .arg("bindings")
            .arg("--library")
            .arg("--ts-dir")
            .arg(ts_dir.as_str())
            .arg("--cpp-dir")
            .arg(rs_dir.as_str())
            .arg(cdylib_path.as_str()),
    );
}

/// Generate `src/lib.rs` from the `.rs` module files in `src_dir`.
///
/// Each `.rs` file is named `{namespace}_module.rs`. The lib.rs includes a
/// `use {library_name};` import and `mod` declarations for each module.
fn generate_lib_rs(src_dir: &Utf8Path, library_name: &str) {
    let mut module_names: Vec<String> = Vec::new();

    let read_dir =
        std::fs::read_dir(src_dir).unwrap_or_else(|e| panic!("failed to read {src_dir}: {e}"));

    for entry in read_dir.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "rs" {
                let stem = path
                    .file_stem()
                    .expect("rs file has no stem")
                    .to_string_lossy()
                    .to_string();
                // Skip lib.rs itself (from a previous run) to avoid circular modules
                if stem != "lib" {
                    module_names.push(stem);
                }
            }
        }
    }

    module_names.sort();

    // The library name uses underscores (Rust identifier)
    let lib_ident = library_name.replace('-', "_");

    let mod_decls: String = module_names
        .iter()
        .map(|m| format!("mod {m};"))
        .collect::<Vec<_>>()
        .join("\n");

    let lib_rs = format!("#[allow(unused_imports)]\nuse {lib_ident};\n\n{mod_decls}\n");

    let lib_rs_path = src_dir.join("lib.rs");
    std::fs::write(&lib_rs_path, lib_rs).expect("failed to write lib.rs");
}

/// Generate `Cargo.toml` from the template with substitutions.
fn generate_cargo_toml(
    wasm_crate_dir: &Utf8Path,
    crate_name: &str,
    package_dir: &Utf8Path,
    src_dir: &Utf8Path,
) {
    let repo_root = paths::repo_root();
    let uniffi_runtime_javascript = repo_root.join("crates/uniffi-runtime-javascript");

    let crate_path = diff_utf8_paths(package_dir, wasm_crate_dir)
        .expect("cannot compute relative path to fixture crate");
    let runtime_path = diff_utf8_paths(uniffi_runtime_javascript, wasm_crate_dir)
        .expect("cannot compute relative path to uniffi-runtime-javascript");
    let lib_rs_path = diff_utf8_paths(src_dir.join("lib.rs"), wasm_crate_dir)
        .expect("cannot compute relative path to lib.rs");

    let cargo_toml_content = CARGO_TEMPLATE
        .replace("{{crate_name}}", crate_name)
        .replace("{{crate_path}}", crate_path.as_str())
        .replace("{{uniffi_runtime_javascript}}", runtime_path.as_str())
        .replace("{{lib_rs_path}}", lib_rs_path.as_str());

    let cargo_toml_path = wasm_crate_dir.join("Cargo.toml");
    std::fs::write(&cargo_toml_path, cargo_toml_content).expect("failed to write Cargo.toml");
}

/// `cargo build --target wasm32-unknown-unknown --manifest-path <path>`
fn compile_wasm32(cargo_toml: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("build")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--manifest-path")
            .arg(cargo_toml.as_str()),
    );
}

/// Run `wasm-bindgen` to produce JS glue from the WASM binary.
fn run_wasm_bindgen(wasm_file: &Utf8Path, out_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("wasm-bindgen")
            .arg("--target")
            .arg("bundler")
            .arg("--omit-default-module-path")
            .arg("--out-name")
            .arg("index")
            .arg("--out-dir")
            .arg(out_dir.as_str())
            .arg(wasm_file.as_str()),
    );
}
