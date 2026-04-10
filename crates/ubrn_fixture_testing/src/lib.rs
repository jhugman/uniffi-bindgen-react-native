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

use std::ffi::OsStr;
use std::process::Command;
use std::sync::Mutex;

use camino::{Utf8Path, Utf8PathBuf};

/// Serialize test flavors within a fixture.
///
/// Both JSI and WASM generate TypeScript bindings into the fixture's
/// `generated/` directory, so tests for different flavors of the same
/// fixture must not run concurrently. Since all tests for a fixture run
/// in the same test binary, an in-process mutex suffices.
static FIXTURE_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn lock_fixture() -> std::sync::MutexGuard<'static, ()> {
    FIXTURE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
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

/// Extension trait to convert paths to forward slashes for use in file content.
///
/// On Windows, paths use backslashes which break certain tools.
pub(crate) trait ForwardSlashPath {
    fn to_forward_slash(&self) -> String;
}

impl ForwardSlashPath for Utf8Path {
    fn to_forward_slash(&self) -> String {
        self.as_str().replace('\\', "/")
    }
}

impl ForwardSlashPath for Utf8PathBuf {
    fn to_forward_slash(&self) -> String {
        self.as_path().to_forward_slash()
    }
}

/// Compute a relative path between two paths, using forward slashes.
///
/// On Windows, `diff_utf8_paths` returns backslash paths which break
/// certain tools
pub(crate) fn relative_path(path: impl AsRef<Utf8Path>, base: impl AsRef<Utf8Path>) -> Utf8PathBuf {
    let rel = pathdiff::diff_utf8_paths(path.as_ref(), base.as_ref()).unwrap_or_else(|| {
        panic!(
            "cannot compute relative path from {} to {}",
            base.as_ref(),
            path.as_ref()
        )
    });
    Utf8PathBuf::from(rel.to_forward_slash())
}

/// Create a [`Command`] that works on Windows for `.cmd`/`.bat` scripts.
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}

/// Run a command, inheriting stdout/stderr so output is visible.
pub(crate) fn run_cmd(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to launch {:?}: {e}", cmd.get_program()));

    if !status.success() {
        panic!("{:?} failed (exit status: {})", cmd.get_program(), status);
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

/// `cargo build -p <crate_name>`
pub(crate) fn cargo_build(crate_name: &str) {
    run_cmd_quietly(
        Command::new("cargo")
            .arg("build")
            .arg("-p")
            .arg(crate_name)
            .arg("--lib"),
    );
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
    let rel_root = relative_path(repo_root, fixture_dir);
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
    let tsx = paths::node_modules_bin().join("tsx");
    run_cmd(
        command(&tsx)
            .arg("--experimental-wasm-modules")
            .arg(test_script.as_str()),
    );
}
