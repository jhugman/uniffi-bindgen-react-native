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
    crate::set_target_dir(target_tmpdir);
    paths::assert_wasm_bootstrap();
    crate::ensure_ubrn_binary();

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");

    let shared_target_dir = Utf8PathBuf::from(target_tmpdir).join("ubrn-tests/wasm-shared-target");
    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/{crate_name}-{test_stem}-wasm"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    let target_dir = crate::target_dir();
    let fixture_dir = metadata::fixture_dir_from_script(test_script);
    let lib_name = metadata::read_cdylib_name(&fixture_dir);
    let cdylib_path = metadata::find_cdylib_path(&lib_name, target_dir);

    let wasm_crate_dir = out_dir.join("wasm-crate");
    let generated_wasm = fixture_dir.join("generated/wasm");
    let ts_dir = generated_wasm.join("ts");
    let wasm_crate_src = generated_wasm.join("rs");

    // Cache codegen: output is a pure function of cdylib content + ubrn binary.
    // Skip regeneration when neither has changed.
    let codegen_stamp_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        crate::file_content_hash(cdylib_path.as_std_path()).hash(&mut hasher);
        // Use ubrn binary mtime (faster than hashing a large binary).
        if let Ok(meta) = std::fs::metadata(crate::ubrn_binary_path().as_std_path()) {
            if let Ok(mtime) = meta.modified() {
                mtime.hash(&mut hasher);
            }
        }
        format!("codegen:{}", hasher.finish())
    };

    let codegen_cached = crate::is_cache_valid(&generated_wasm, &codegen_stamp_key)
        && ts_dir.exists()
        && wasm_crate_src.exists();

    if !codegen_cached {
        let temp_dir = out_dir.join("gen-tmp-wasm");
        let temp_ts = temp_dir.join("ts");
        let temp_rs = temp_dir.join("rs");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_rs).expect("failed to create temp rs dir");
        generate_bindings(&cdylib_path, &temp_ts, &temp_rs);
        crate::sync_dir_write_if_changed(&temp_ts, &ts_dir);
        crate::sync_dir_write_if_changed(&temp_rs, &wasm_crate_src);
        let _ = std::fs::remove_dir_all(&temp_dir);
        generate_lib_rs(&wasm_crate_src, &lib_name);
        crate::write_cache_stamp(&generated_wasm, &codegen_stamp_key);
    }

    // Hash the generated output — only rebuild if content actually changed.
    let rs_hash = crate::dir_content_hash(&wasm_crate_src);
    let ts_hash = crate::dir_content_hash(&ts_dir);
    let stamp_key = format!("wasm-build:{rs_hash}:{ts_hash}");

    let wasm_bindgen_index = fixture_dir.join("generated/wasm/ts/wasm-bindgen/index_bg.wasm");

    if !(crate::is_cache_valid(&out_dir, &stamp_key) && wasm_bindgen_index.exists()) {
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

        let cargo_toml = wasm_crate_dir.join("Cargo.toml");
        compile_wasm32(&cargo_toml, &shared_target_dir);

        let wasm_file = shared_target_dir.join("wasm32-unknown-unknown/debug/my_test_crate.wasm");
        let wasm_bindgen_dir = ts_dir.join("wasm-bindgen");
        std::fs::create_dir_all(&wasm_bindgen_dir).expect("failed to create wasm-bindgen dir");
        run_wasm_bindgen(&wasm_file, &wasm_bindgen_dir);

        crate::write_cache_stamp(&out_dir, &stamp_key);
    }

    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &fixture_dir,
        crate::Flavor::Wasm,
    ));
    crate::run_tsx_in_dir(test_script, Some(&fixture_dir));
}

/// Generate bindings via the CLI binary directly.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path, rs_dir: &Utf8Path) {
    let binary = crate::ensure_ubrn_binary();
    run_cmd_quietly(
        Command::new(binary.as_str())
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
fn generate_lib_rs(src_dir: &Utf8Path, library_name: &str) {
    let module_names: Vec<String> = crate::collect_file_stems(src_dir, "rs")
        .into_iter()
        .filter(|s| s != "lib")
        .collect();
    let lib_ident = library_name.replace('-', "_");
    let mod_decls: String = module_names
        .iter()
        .map(|m| format!("mod {m};"))
        .collect::<Vec<_>>()
        .join("\n");
    let lib_rs = format!("#[allow(unused_imports)]\nuse {lib_ident};\n\n{mod_decls}\n");
    crate::write_file_if_changed(&src_dir.join("lib.rs"), &lib_rs);
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
    crate::write_file_if_changed(&wasm_crate_dir.join("Cargo.toml"), &cargo_toml_content);
}

fn compile_wasm32(cargo_toml: &Utf8Path, shared_target_dir: &Utf8Path) {
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let jobs = std::cmp::max(1, num_cpus / 3);
    run_cmd_quietly(
        Command::new("cargo")
            .arg("build")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--manifest-path")
            .arg(cargo_toml.as_str())
            .arg("--jobs")
            .arg(jobs.to_string())
            .env("CARGO_TARGET_DIR", shared_target_dir.as_str()),
    );
}

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
