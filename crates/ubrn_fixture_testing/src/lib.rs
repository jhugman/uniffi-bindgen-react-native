/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod metadata;
mod paths;
pub mod typescript;

pub mod jsi;
pub mod ts;
pub mod wasm;

/// Test flavor: JSI (Hermes native) or WASM (Node.js).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Jsi,
    Wasm,
}

impl Flavor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Flavor::Jsi => "jsi",
            Flavor::Wasm => "wasm",
        }
    }
}

use std::process::Command;

/// Ensure the `uniffi-bindgen-react-native` binary is built and up-to-date.
///
/// Runs `cargo build` once per process (cached by LazyLock).
pub(crate) fn ensure_ubrn_binary() -> &'static camino::Utf8Path {
    use std::sync::LazyLock;
    static BIN: LazyLock<camino::Utf8PathBuf> = LazyLock::new(|| {
        run_cmd_quietly(
            Command::new("cargo")
                .arg("build")
                .arg("-p")
                .arg("uniffi-bindgen-react-native")
                .arg("--quiet"),
        );
        let bin = target_dir()
            .join("debug")
            .join("uniffi-bindgen-react-native");
        assert!(
            bin.exists(),
            "uniffi-bindgen-react-native binary not found at {bin}."
        );
        bin
    });
    &BIN
}

/// Store the target directory derived from `CARGO_TARGET_TMPDIR`.
///
/// Called once per process from the first `run_test()` invocation.
static TARGET_DIR: std::sync::OnceLock<camino::Utf8PathBuf> = std::sync::OnceLock::new();

pub(crate) fn set_target_dir(target_tmpdir: &str) {
    TARGET_DIR.get_or_init(|| metadata::target_dir_from_tmpdir(target_tmpdir));
}

pub(crate) fn target_dir() -> &'static camino::Utf8Path {
    TARGET_DIR
        .get()
        .expect("target_dir not initialized; call set_target_dir first")
}

/// RAII guard that removes a file when dropped, ensuring cleanup even on panic.
pub(crate) struct CleanupFile(camino::Utf8PathBuf);

impl CleanupFile {
    pub(crate) fn new(path: camino::Utf8PathBuf) -> Self {
        Self(path)
    }
}

impl Drop for CleanupFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Run a command, capturing stdout/stderr. Only display output on failure.
pub(crate) fn run_cmd_quietly(cmd: &mut Command) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to launch {:?}: {e}", cmd.get_program()));

    if !output.status.success() {
        eprintln!("Command failed: {cmd:?}");
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!(
            "{:?} failed (exit status: {})",
            cmd.get_program(),
            output.status
        );
    }
}

/// Run a command with stdout/stderr visible during `--nocapture`, captured otherwise.
///
/// Used for test-runner execution where `console.log` should be visible
/// when running with `cargo test -- --nocapture`.
///
/// Without `--nocapture`, Rust's test harness captures the test thread's stdout.
/// We mirror this by using `.output()` (captured) normally, and `.status()`
/// (inherited stdio) when nocapture is active.
pub(crate) fn run_cmd(cmd: &mut Command) {
    // Rust's test harness passes --nocapture through as a test binary argument.
    let nocapture = std::env::args().any(|a| a == "--nocapture")
        || std::env::var("RUST_TEST_NOCAPTURE").is_ok();

    if nocapture {
        let status = cmd
            .status()
            .unwrap_or_else(|e| panic!("failed to launch {:?}: {e}", cmd.get_program()));
        if !status.success() {
            panic!("{:?} failed (exit status: {})", cmd.get_program(), status);
        }
    } else {
        run_cmd_quietly(cmd);
    }
}

/// Sync files from `src` to `dst` using write-if-changed semantics.
///
/// For each file in `src`:
///   - If the same file in `dst` has identical content, skip (preserving mtime).
///   - Otherwise, copy the file (updating mtime).
///
/// Files in `dst` that don't exist in `src` are removed (stale outputs).
///
/// This preserves mtimes of unchanged files so that downstream tools
/// (ninja, tsc incremental) can skip unnecessary rebuilds.
pub(crate) fn sync_dir_write_if_changed(src: &camino::Utf8Path, dst: &camino::Utf8Path) {
    std::fs::create_dir_all(dst).expect("failed to create dst dir");

    // Collect filenames present in src
    let mut src_names = std::collections::HashSet::new();

    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy().to_string();
            src_names.insert(fname_str.clone());

            let src_path = camino::Utf8PathBuf::try_from(entry.path()).expect("non-UTF-8 src path");
            let dst_path = dst.join(&fname_str);

            if src_path.is_dir() {
                let dst_sub = dst.join(&fname_str);
                sync_dir_write_if_changed(&src_path, &dst_sub);
                continue;
            }

            // Read src content
            let src_content = std::fs::read(&src_path)
                .unwrap_or_else(|e| panic!("failed to read {src_path}: {e}"));

            // Compare with dst — skip if identical to preserve mtime
            if let Ok(dst_content) = std::fs::read(&dst_path) {
                if src_content == dst_content {
                    continue;
                }
            }

            // Content differs or dst doesn't exist — write
            std::fs::write(&dst_path, &src_content)
                .unwrap_or_else(|e| panic!("failed to write {dst_path}: {e}"));
        }
    }

    // Remove stale files in dst that aren't in src
    if let Ok(entries) = std::fs::read_dir(dst) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if !src_names.contains(&fname) {
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

/// Write `content` to `path` only if the file doesn't already contain identical content.
///
/// Preserves the file's mtime when content is unchanged, so downstream tools
/// (ninja, cargo, tsc) can skip unnecessary rebuilds.
pub(crate) fn write_file_if_changed(path: &camino::Utf8Path, content: &str) {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == content {
            return;
        }
    }
    std::fs::write(path, content).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
}

/// Collect sorted file stems from `dir` that have the given `extension`.
pub(crate) fn collect_file_stems(dir: &camino::Utf8Path, extension: &str) -> Vec<String> {
    let read_dir = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("failed to read {dir}: {e}"));
    let mut stems: Vec<String> = read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let ext = path.extension()?;
            if ext == extension {
                Some(
                    path.file_stem()
                        .expect("file with extension has no stem")
                        .to_string_lossy()
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect();
    stems.sort();
    stems
}

/// Write a minimal tsconfig.json into the fixture directory so that tsx
/// can resolve `@generated/*` and `@/*` imports.
///
/// `@generated/*` resolves to `./generated/$flavor/ts/*`.
pub(crate) fn write_fixture_tsconfig(
    fixture_dir: &camino::Utf8Path,
    flavor: Flavor,
) -> camino::Utf8PathBuf {
    let flavor = flavor.as_str();
    let repo_root = paths::repo_root();
    let rel_root = pathdiff::diff_utf8_paths(repo_root, fixture_dir)
        .expect("cannot compute relative path to repo root");
    let tsconfig_path = fixture_dir.join("tsconfig.json");
    let contents = format!(
        r#"{{
  "compilerOptions": {{
    "baseUrl": ".",
    "paths": {{
      "@/generated/*": ["./generated/{flavor}/ts/*"],
      "@/*": ["{rel_root}/typescript/testing/*"],
      "uniffi-bindgen-react-native": ["{rel_root}/typescript/src/index"]
    }}
  }}
}}
"#
    );
    std::fs::write(&tsconfig_path, contents).expect("failed to write fixture tsconfig.json");
    tsconfig_path
}

/// Run a test script with tsx and experimental WASM module support.
pub(crate) fn run_tsx(test_script: &camino::Utf8Path) {
    run_tsx_in_dir(test_script, None);
}

/// Run a test script with tsx, optionally setting the working directory.
///
/// Uses `node --import tsx` instead of the `tsx` binary for faster startup
/// (~95ms savings per invocation).
///
/// The `cwd` parameter is needed for fixture WASM tests: tsx resolves
/// `tsconfig.json` paths relative to CWD, so we must run from the fixture
/// directory where the tsconfig lives.
pub(crate) fn run_tsx_in_dir(test_script: &camino::Utf8Path, cwd: Option<&camino::Utf8Path>) {
    let mut cmd = Command::new("node");
    cmd.arg("--import")
        .arg("tsx")
        .arg("--experimental-wasm-modules")
        .arg(test_script.as_str());
    if let Some(dir) = cwd {
        cmd.current_dir(dir.as_std_path());
    }
    run_cmd(&mut cmd);
}
