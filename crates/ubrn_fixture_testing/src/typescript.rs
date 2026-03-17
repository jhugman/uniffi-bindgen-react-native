/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// Shared TypeScript compilation utilities (tsc, tsc-alias, metro).

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use pathdiff::diff_utf8_paths;

use crate::{paths, run_cmd_quietly};

/// Full JSI preparation: compile TS → rewrite paths → bundle with Metro.
/// Returns the path to the Metro bundle.
///
/// `generated_dir` is the fixture's `generated/` directory, used to set up
/// the `@generated/*` tsconfig path mapping. Pass `None` for framework tests
/// that don't have a fixture.
pub fn prepare_for_jsi(
    test_script: &Utf8Path,
    out_dir: &Utf8Path,
    generated_dir: Option<&Utf8Path>,
) -> Utf8PathBuf {
    let stem = test_script.file_stem().unwrap_or("test");
    let tsc_dir = out_dir.join("tsc");

    // Generate tsconfig.json
    let tsconfig = prepare_tsconfig(&tsc_dir, "es5", test_script, generated_dir);

    // Compile with tsc
    compile_ts(&tsconfig);

    // Rewrite path aliases in the compiled JS files.
    // tsc-alias has trouble when outDir == configDir, so we do it ourselves.
    rewrite_at_paths(&tsc_dir, generated_dir);

    // Find the compiled JS file
    let js_file = find_compiled_js(&tsc_dir, stem);

    // Bundle with Metro
    let bundle_dir = out_dir.join("bundles");
    std::fs::create_dir_all(&bundle_dir).expect("failed to create bundle dir");
    let bundle_path = bundle_dir.join(format!("{stem}.bundle.js"));
    bundle_with_metro(&js_file, &bundle_path, &tsc_dir);

    bundle_path
}

/// Generate a tsconfig.json in `tsc_dir` from the workspace template.
///
/// The `target` is an ECMAScript target string, e.g. "es5" or "ES2024".
/// The `test_script` is included in the `"files"` array so tsc knows what to compile.
/// Returns the path to the generated tsconfig.json.
fn prepare_tsconfig(
    tsc_dir: &Utf8Path,
    target: &str,
    test_script: &Utf8Path,
    generated_dir: Option<&Utf8Path>,
) -> Utf8PathBuf {
    std::fs::create_dir_all(tsc_dir).expect("failed to create tsc output dir");

    let repo_root = paths::repo_root();
    let template_path = repo_root.join("typescript").join("tsconfig.template.json");

    let template =
        std::fs::read_to_string(&template_path).expect("failed to read tsconfig.template.json");

    // Compute a relative path from tsc_dir back to the repo root so the
    // `paths` aliases in the template resolve correctly.
    let rel_root =
        diff_utf8_paths(repo_root, tsc_dir).expect("cannot compute relative path to repo root");

    let mut contents = template
        .replace("{{repository_root}}", rel_root.as_str())
        .replace("{{target}}", target)
        // Uncomment baseUrl so that tsc-alias can resolve path aliases.
        .replace("// \"baseUrl\": \"./\",", "\"baseUrl\": \"./\",");

    // Add @generated/* path mapping if a generated dir was provided.
    if let Some(gen_dir) = generated_dir {
        let rel_gen = diff_utf8_paths(gen_dir, tsc_dir)
            .expect("cannot compute relative path to generated dir");
        contents = contents.replace(
            "\"@/*\":",
            &format!("\"@/generated/*\": [\"{rel_gen}/*\"],\n      \"@/*\":"),
        );
    }

    // Uncomment outDir so tsc-alias can find the output directory.
    // Use "." (relative to tsconfig location = tsc_dir) so tsc-alias doesn't
    // double the absolute path when resolving.
    let contents = contents.replace("// \"outDir\": \"./\",", "\"outDir\": \".\",");

    // Add a "files" array with the test script.  The template uses jsonc
    // (comments), so we do a simple approach: strip the trailing `}` and
    // append the files array.
    let contents = contents.trim_end().trim_end_matches('}');
    let contents = format!("{contents},\n  \"files\": [\"{}\"]\n}}\n", test_script,);

    // Write the tsconfig into tsc_dir (the output directory) to avoid polluting the source tree.
    let tsconfig = tsc_dir.join("tsconfig.json");
    std::fs::write(&tsconfig, contents).expect("failed to write tsconfig.json");

    tsconfig
}

/// Run `tsc --project <tsconfig>`.
///
/// The test script is listed in the tsconfig's `"files"` array and `outDir` is
/// set in the tsconfig, so no extra CLI flags are needed beyond `--project`.
fn compile_ts(tsconfig: &Utf8Path) {
    let tsc = paths::node_modules_bin().join("tsc");
    run_cmd_quietly(Command::new(&tsc).arg("--project").arg(tsconfig));
}

/// Rewrite tsconfig path aliases in all JS files under `tsc_dir`.
///
/// Handles these aliases from the tsconfig:
///  - `@/*`                        → `typescript/testing/*`
///  - `@/generated/*`              → fixture's `generated/*` (if provided)
///  - `uniffi-bindgen-react-native` → `typescript/src/index`
///
/// After tsc compilation the directory structure is preserved under `tsc_dir`,
/// so we compute relative paths per-file and do string replacement.
fn rewrite_at_paths(tsc_dir: &Utf8Path, generated_dir: Option<&Utf8Path>) {
    let testing_dir = tsc_dir.join("typescript/testing");
    let ubrn_index = tsc_dir.join("typescript/src/index");
    // tsc preserves the source directory structure under outDir relative to
    // a common root. The generated files end up mirrored under tsc_dir.
    let generated_in_tsc = generated_dir.map(|gen_dir| {
        let repo_root = paths::repo_root();
        let rel = diff_utf8_paths(gen_dir, repo_root)
            .expect("cannot compute relative path from repo root to generated dir");
        tsc_dir.join(rel)
    });
    rewrite_paths_recursive(
        tsc_dir,
        &testing_dir,
        &ubrn_index,
        generated_in_tsc.as_deref(),
    );
}

fn rewrite_paths_recursive(
    dir: &Utf8Path,
    testing_dir: &Utf8Path,
    ubrn_index: &Utf8Path,
    generated_dir: Option<&Utf8Path>,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let utf8: Utf8PathBuf = match path.try_into() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if utf8.is_dir() {
            rewrite_paths_recursive(&utf8, testing_dir, ubrn_index, generated_dir);
        } else if utf8.extension() == Some("js") {
            let contents = match std::fs::read_to_string(&utf8) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let needs_generated = contents.contains("\"@/generated/");
            let needs_at = contents.contains("\"@/");
            let needs_ubrn = contents.contains("\"uniffi-bindgen-react-native\"");
            if !needs_at && !needs_generated && !needs_ubrn {
                continue;
            }

            let file_dir = utf8.parent().unwrap();
            let mut rewritten = contents;

            // Rewrite @/generated/ BEFORE @/ so the more specific pattern
            // matches first (both start with "@/").
            if needs_generated {
                if let Some(gen_dir) = generated_dir {
                    let rel = diff_utf8_paths(gen_dir, file_dir).unwrap_or_else(|| {
                        panic!("cannot compute relative path from {file_dir} to {gen_dir}")
                    });
                    let rel_str = make_relative(&rel);
                    rewritten = rewritten.replace("\"@/generated/", &format!("\"{rel_str}/"));
                }
            }

            if needs_at {
                let rel = diff_utf8_paths(testing_dir, file_dir).unwrap_or_else(|| {
                    panic!("cannot compute relative path from {file_dir} to {testing_dir}")
                });
                let rel_str = make_relative(&rel);
                rewritten = rewritten.replace("\"@/", &format!("\"{rel_str}/"));
            }

            if needs_ubrn {
                let rel = diff_utf8_paths(ubrn_index, file_dir).unwrap_or_else(|| {
                    panic!("cannot compute relative path from {file_dir} to {ubrn_index}")
                });
                let rel_str = make_relative(&rel);
                rewritten =
                    rewritten.replace("\"uniffi-bindgen-react-native\"", &format!("\"{rel_str}\""));
            }

            std::fs::write(&utf8, rewritten)
                .unwrap_or_else(|e| panic!("failed to write {utf8}: {e}"));
        }
    }
}

/// Ensure a relative path starts with "./" or "../" so Node treats it as
/// a file path rather than a bare module specifier.
fn make_relative(rel: &Utf8Path) -> String {
    if rel.as_str().starts_with("..") {
        rel.to_string()
    } else {
        format!("./{rel}")
    }
}

/// Locate `{stem}.js` anywhere under `tsc_dir` using a recursive walk.
fn find_compiled_js(tsc_dir: &Utf8Path, stem: &str) -> Utf8PathBuf {
    let filename = format!("{stem}.js");
    find_file_recursive(tsc_dir, &filename)
        .unwrap_or_else(|| panic!("compiled JS file {filename} not found under {tsc_dir}"))
}

fn find_file_recursive(dir: &Utf8Path, filename: &str) -> Option<Utf8PathBuf> {
    let read_dir = std::fs::read_dir(dir).ok()?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        let utf8: Utf8PathBuf = match path.try_into() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if utf8.is_dir() {
            if let Some(found) = find_file_recursive(&utf8, filename) {
                return Some(found);
            }
        } else if utf8.file_name() == Some(filename) {
            return Some(utf8);
        }
    }
    None
}

/// Run `metro build --minify false --out <bundle_path> <js_file>`.
///
/// A temporary `metro.config.js` is generated in `tsc_dir` so that Metro can
/// discover files that live under `target/` (which watchman normally ignores
/// because it is listed in `.gitignore`).
fn bundle_with_metro(js_file: &Utf8Path, bundle_path: &Utf8Path, tsc_dir: &Utf8Path) {
    let metro = paths::node_modules_bin().join("metro");
    let repo_root = paths::repo_root();

    // Generate a metro config that:
    //  - adds tsc_dir to watchFolders (files live under target/ which
    //    watchman ignores because it is in .gitignore)
    //  - disables watchman for the same reason
    //  - resolves `@/*` imports to the compiled `typescript/testing/*` tree
    //    (tsc-alias cannot reliably rewrite these when outDir == configDir)
    let testing_dir = tsc_dir.join("typescript/testing");
    let metro_config_path = tsc_dir.join("metro.config.cjs");
    let metro_config = format!(
        r#"const path = require("path");
module.exports = {{
  projectRoot: path.resolve("{repo_root}"),
  watchFolders: [path.resolve("{tsc_dir}")],
  resolver: {{
    useWatchman: false,
    extraNodeModules: {{
      "@": path.resolve("{testing_dir}"),
    }},
  }},
}};
"#,
    );
    std::fs::write(&metro_config_path, metro_config).expect("failed to write metro.config.js");

    run_cmd_quietly(
        Command::new(&metro)
            .arg("build")
            .arg("--minify")
            .arg("false")
            .arg("--config")
            .arg(&metro_config_path)
            .arg("--out")
            .arg(bundle_path)
            .arg(js_file),
    );
}
