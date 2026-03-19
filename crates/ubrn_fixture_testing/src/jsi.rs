/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use heck::ToUpperCamelCase;

use crate::{metadata, paths, run_cmd_quietly, typescript};

/// Run a fixture test under the JSI (Hermes) flavor.
///
/// Called from proc-macro-generated `#[test]` functions.
pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();

    // Step 0: Check bootstrap
    paths::assert_jsi_bootstrap();

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");

    // Per-test output directory
    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/{crate_name}-{test_stem}-jsi"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    // Step 1: Build the fixture crate
    crate::cargo_build(crate_name);

    // Step 2: Generate bindings
    // All generated artifacts go under fixture's generated/jsi/ for easy inspection.
    // Test scripts use @generated/* imports which are resolved via tsconfig.
    let lib_name = metadata::find_cdylib_name(crate_name);
    let cdylib_path = metadata::find_cdylib_from_name(&lib_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated_jsi = fixture_dir.join("generated/jsi");
    let _ = std::fs::remove_dir_all(&generated_jsi);
    let ts_dir = generated_jsi.join("ts");
    let cpp_dir = generated_jsi.join("cpp");
    generate_bindings(&cdylib_path, &ts_dir, &cpp_dir);

    // Step 3: Generate Entrypoint.cpp
    generate_entrypoint(&cpp_dir);

    // Step 4: Compile C++ with CMake/Ninja
    let target_dir = &metadata::workspace_metadata().target_directory;
    let so_file = compile_cpp(&cpp_dir, &out_dir, &lib_name, target_dir);

    // Step 5: Compile TypeScript and bundle
    let bundle = typescript::prepare_for_jsi(test_script, &out_dir, Some(&ts_dir));

    // Step 6: Run test-runner
    run_test_runner(&bundle, &so_file);
}

/// Generate bindings via the CLI.
///
/// Uses `--library` mode which auto-discovers per-crate configs, so no
/// separate `--config` flag is needed (and they conflict in the CLI).
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path, cpp_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("uniffi-bindgen-react-native")
            .arg("--")
            .arg("generate")
            .arg("jsi")
            .arg("bindings")
            .arg("--library")
            .arg("--ts-dir")
            .arg(ts_dir.as_str())
            .arg("--cpp-dir")
            .arg(cpp_dir.as_str())
            .arg(cdylib_path.as_str()),
    );
}

/// Generate `Entrypoint.cpp` from the `.hpp` files in `cpp_dir`.
fn generate_entrypoint(cpp_dir: &Utf8Path) {
    let mut hpp_files: Vec<String> = Vec::new();
    let mut module_names: Vec<String> = Vec::new();

    let read_dir =
        std::fs::read_dir(cpp_dir).unwrap_or_else(|e| panic!("failed to read {cpp_dir}: {e}"));

    for entry in read_dir.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "hpp" {
                let stem = path
                    .file_stem()
                    .expect("hpp file has no stem")
                    .to_string_lossy()
                    .to_string();
                hpp_files.push(stem.clone());
                let camel = stem.to_upper_camel_case();
                module_names.push(format!("Native{camel}"));
            }
        }
    }

    hpp_files.sort();
    module_names.sort();

    let includes: String = hpp_files
        .iter()
        .map(|f| format!("#include \"{f}.hpp\""))
        .collect::<Vec<_>>()
        .join("\n");

    let registrations: String = module_names
        .iter()
        .map(|m| format!("    {m}::registerModule(rt, callInvoker);"))
        .collect::<Vec<_>>()
        .join("\n");

    let entrypoint = format!(
        r#"#include "registerNatives.h"
{includes}

extern "C" void registerNatives(jsi::Runtime &rt, std::shared_ptr<react::CallInvoker> callInvoker) {{
{registrations}
}}
"#
    );

    let entrypoint_path = cpp_dir.join("Entrypoint.cpp");
    std::fs::write(&entrypoint_path, entrypoint).expect("failed to write Entrypoint.cpp");
}

/// Compile C++ with CMake/Ninja, returning the path to the shared library.
fn compile_cpp(
    cpp_dir: &Utf8Path,
    out_dir: &Utf8Path,
    lib_name: &str,
    target_dir: &Utf8Path,
) -> Utf8PathBuf {
    let build_dir = out_dir.join("cpp-build");
    std::fs::create_dir_all(&build_dir).expect("failed to create cpp-build dir");

    let repo_root = paths::repo_root();

    // Collect all .cpp files in cpp_dir
    let mut cpp_files: Vec<String> = Vec::new();
    let read_dir =
        std::fs::read_dir(cpp_dir).unwrap_or_else(|e| panic!("failed to read {cpp_dir}: {e}"));

    for entry in read_dir.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "cpp" {
                let canonical = path
                    .canonicalize()
                    .expect("failed to canonicalize cpp path");
                cpp_files.push(canonical.to_string_lossy().to_string());
            }
        }
    }

    cpp_files.sort();
    let cpp_files_str = cpp_files.join(";");

    let cmake_lists_dir = repo_root.join("cpp/hermes-rust-extension");

    // Run cmake
    run_cmd_quietly(
        Command::new("cmake")
            .arg("-G")
            .arg("Ninja")
            .arg(format!("-DHERMES_SRC_DIR={}", paths::hermes_src_dir()))
            .arg(format!("-DHERMES_BUILD_DIR={}", paths::hermes_build_dir()))
            .arg(format!("-DHERMES_EXTENSION_NAME=rn-{lib_name}"))
            .arg(format!("-DRUST_LIB_NAME={lib_name}"))
            .arg(format!("-DRUST_TARGET_DIR={}/debug", target_dir))
            .arg(format!("-DHERMES_EXTENSION_CPP={cpp_files_str}"))
            .arg(cmake_lists_dir.as_str())
            .current_dir(&build_dir),
    );

    // Run ninja
    run_cmd_quietly(Command::new("ninja").arg("-C").arg(build_dir.as_str()));

    build_dir.join(format!("librn-{lib_name}.{}", metadata::shared_lib_ext()))
}

/// Run the test-runner binary.
fn run_test_runner(bundle: &Utf8Path, so_file: &Utf8Path) {
    let runner = paths::test_runner_binary();
    run_cmd_quietly(
        Command::new(runner.as_str())
            .arg(bundle.as_str())
            .arg(so_file.as_str()),
    );
}
