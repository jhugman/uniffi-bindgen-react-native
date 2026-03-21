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
    crate::set_target_dir(target_tmpdir);
    paths::assert_jsi_bootstrap();
    crate::ensure_ubrn_binary();

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");

    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/{crate_name}-{test_stem}-jsi"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    let target_dir = crate::target_dir();
    let fixture_dir = metadata::fixture_dir_from_script(test_script);
    let lib_name = metadata::read_cdylib_name(&fixture_dir);
    let cdylib_path = metadata::find_cdylib_path(&lib_name, target_dir);

    let so_file = out_dir
        .join("cpp-build")
        .join(format!("librn-{lib_name}.{}", metadata::shared_lib_ext()));
    let bundle = out_dir
        .join("bundles")
        .join(format!("{test_stem}.bundle.js"));

    // Codegen output cached by cdylib content + ubrn binary mtime.
    // On cache miss, write-if-changed sync preserves mtimes when output
    // is identical, so downstream tools (ninja, tsc) skip unnecessary work.
    let generated_jsi = fixture_dir.join("generated/jsi");
    let ts_dir = generated_jsi.join("ts");
    let cpp_dir = generated_jsi.join("cpp");

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

    let codegen_cached = crate::is_cache_valid(&generated_jsi, &codegen_stamp_key)
        && ts_dir.exists()
        && cpp_dir.exists();

    if !codegen_cached {
        let temp_dir = out_dir.join("gen-tmp-jsi");
        let temp_ts = temp_dir.join("ts");
        let temp_cpp = temp_dir.join("cpp");
        let _ = std::fs::remove_dir_all(&temp_dir);
        generate_bindings(&cdylib_path, &temp_ts, &temp_cpp);
        crate::sync_dir_write_if_changed(&temp_ts, &ts_dir);
        crate::sync_dir_write_if_changed(&temp_cpp, &cpp_dir);
        let _ = std::fs::remove_dir_all(&temp_dir);
        generate_entrypoint(&cpp_dir);
        crate::write_cache_stamp(&generated_jsi, &codegen_stamp_key);
    }

    // Hash the generated output — only recompile if content actually changed.
    let cpp_hash = crate::dir_content_hash(&cpp_dir);
    let ts_hash = crate::dir_content_hash(&ts_dir);
    let script_hash = crate::file_content_hash(test_script.as_std_path());

    let (so_file, bundle) = std::thread::scope(|s| {
        let cpp_handle = s.spawn(|| {
            let stamp_key = format!("cpp-build:{cpp_hash}");
            if crate::is_cache_valid(&out_dir, &stamp_key) && so_file.exists() {
                so_file.clone()
            } else {
                let result = compile_cpp(&cpp_dir, &out_dir, &lib_name, &cdylib_path);
                crate::write_cache_stamp(&out_dir, &stamp_key);
                result
            }
        });
        let ts_handle = s.spawn(|| {
            let stamp_key = format!("ts-build:{script_hash}:{ts_hash}");
            let stamp_dir = out_dir.join("tsc");
            if crate::is_cache_valid(&stamp_dir, &stamp_key) && bundle.exists() {
                bundle.clone()
            } else {
                let result = typescript::prepare_for_jsi(test_script, &out_dir, Some(&ts_dir));
                crate::write_cache_stamp(&stamp_dir, &stamp_key);
                result
            }
        });
        (cpp_handle.join().unwrap(), ts_handle.join().unwrap())
    });

    run_test_runner(&bundle, &so_file);
}

/// Generate bindings via the CLI binary directly.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path, cpp_dir: &Utf8Path) {
    let binary = crate::ensure_ubrn_binary();
    run_cmd_quietly(
        Command::new(binary.as_str())
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
    let hpp_files = crate::collect_file_stems(cpp_dir, "hpp");

    let module_names: Vec<String> = {
        let mut names: Vec<_> = hpp_files
            .iter()
            .map(|stem| format!("Native{}", stem.to_upper_camel_case()))
            .collect();
        names.sort();
        names
    };

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

    crate::write_file_if_changed(&cpp_dir.join("Entrypoint.cpp"), &entrypoint);
}

/// Compile C++ with CMake/Ninja, returning the path to the shared library.
fn compile_cpp(
    cpp_dir: &Utf8Path,
    out_dir: &Utf8Path,
    lib_name: &str,
    cdylib_path: &Utf8Path,
) -> Utf8PathBuf {
    let build_dir = out_dir.join("cpp-build");
    std::fs::create_dir_all(&build_dir).expect("failed to create cpp-build dir");

    let repo_root = paths::repo_root();

    let cpp_stems = crate::collect_file_stems(cpp_dir, "cpp");
    let cpp_files: Vec<String> = cpp_stems
        .iter()
        .map(|stem| {
            let path = cpp_dir.join(format!("{stem}.cpp"));
            std::path::Path::new(path.as_str())
                .canonicalize()
                .expect("failed to canonicalize cpp path")
                .to_string_lossy()
                .to_string()
        })
        .collect();
    let cpp_files_str = cpp_files.join(";");

    let cmake_lists_dir = repo_root.join("cpp/hermes-rust-extension");

    // Skip cmake configure if build.ninja exists and cmake inputs haven't changed.
    // CMake configure validates compilers, parses CMakeLists, checks paths — all
    // deterministic for identical inputs. Ninja handles its own incrementality.
    let cmake_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        crate::file_content_hash(cmake_lists_dir.join("CMakeLists.txt").as_std_path())
            .hash(&mut hasher);
        paths::hermes_src_dir().as_str().hash(&mut hasher);
        paths::hermes_build_dir().as_str().hash(&mut hasher);
        lib_name.hash(&mut hasher);
        cdylib_path.as_str().hash(&mut hasher);
        cpp_files_str.hash(&mut hasher);
        format!("cmake-configure:{}", hasher.finish())
    };

    let build_ninja = build_dir.join("build.ninja");
    if !(build_ninja.exists() && crate::is_cache_valid(&build_dir, &cmake_key)) {
        run_cmd_quietly(
            Command::new("cmake")
                .arg("-G")
                .arg("Ninja")
                .arg(format!("-DHERMES_SRC_DIR={}", paths::hermes_src_dir()))
                .arg(format!("-DHERMES_BUILD_DIR={}", paths::hermes_build_dir()))
                .arg(format!("-DHERMES_EXTENSION_NAME=rn-{lib_name}"))
                .arg(format!("-DRUST_CDYLIB={cdylib_path}"))
                .arg(format!("-DHERMES_EXTENSION_CPP={cpp_files_str}"))
                .arg(cmake_lists_dir.as_str())
                .current_dir(&build_dir),
        );
        crate::write_cache_stamp(&build_dir, &cmake_key);
    }

    run_cmd_quietly(Command::new("ninja").arg("-C").arg(build_dir.as_str()));

    build_dir.join(format!("librn-{lib_name}.{}", metadata::shared_lib_ext()))
}

/// Run the test-runner binary.
fn run_test_runner(bundle: &Utf8Path, so_file: &Utf8Path) {
    let runner = paths::test_runner_binary();
    crate::run_cmd(
        Command::new(runner.as_str())
            .arg(bundle.as_str())
            .arg(so_file.as_str()),
    );
}
