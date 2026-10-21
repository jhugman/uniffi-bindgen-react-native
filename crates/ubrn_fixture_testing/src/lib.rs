/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod metadata;
mod paths;
pub mod typescript;

pub mod jsi;
pub mod jsi2;
pub mod napi;
pub mod ts;
pub mod wasm;
pub mod wasm2;

/// Test flavor: JSI (Hermes native), Jsi2 (generic JSI player shim), WASM
/// (Node.js), Napi (Node.js N-API), or Wasm2 (player-based WASM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Jsi,
    Jsi2,
    Wasm,
    Napi,
    Wasm2,
}

impl Flavor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Flavor::Jsi => "jsi",
            Flavor::Jsi2 => "jsi2",
            Flavor::Wasm => "wasm",
            Flavor::Napi => "napi",
            Flavor::Wasm2 => "wasm2",
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
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg(crate_name).arg("--lib");
    if crate::metadata::cargo_profile() == "release" {
        cmd.arg("--release");
    }
    run_cmd_quietly(&mut cmd);
}

/// Check the selected test and generated bindings before executing them with tsx.
/// Configs live in a per-test output directory, never in the fixture source root.
pub(crate) fn prepare_tsx(
    test_script: &Utf8Path,
    out_dir: &Utf8Path,
    generated_dir: Option<&Utf8Path>,
    flavor: Flavor,
) -> Utf8PathBuf {
    let runtime_config = write_tsx_config(test_script, out_dir, generated_dir, flavor, false);
    let check_config = write_tsx_config(test_script, out_dir, generated_dir, flavor, true);
    run_cmd_quietly(
        command(paths::node_modules_bin().join("tsc"))
            .arg("--noEmit")
            .arg("--project")
            .arg(check_config),
    );
    runtime_config
}

fn write_tsx_config(
    test_script: &Utf8Path,
    out_dir: &Utf8Path,
    generated_dir: Option<&Utf8Path>,
    flavor: Flavor,
    type_check: bool,
) -> Utf8PathBuf {
    std::fs::create_dir_all(out_dir).expect("failed to create TypeScript config directory");
    let rel_root = relative_path(paths::repo_root(), out_dir);
    let runtime_paths = tsconfig_runtimes(flavor, &rel_root, type_check);
    let (generated_paths, include) = match generated_dir {
        Some(dir) => {
            let dir = relative_path(dir, out_dir);
            (
                format!(r#""@/generated": ["{dir}"], "@/generated/*": ["{dir}/*"],"#),
                format!(r#""{dir}/**/*.ts""#),
            )
        }
        None => (String::new(), String::new()),
    };
    let test_script = relative_path(test_script, out_dir);
    let libs = if flavor == Flavor::Napi {
        r#""ES2024""#
    } else {
        r#""ES2024", "DOM""#
    };
    let contents = format!(
        r#"{{
  "compilerOptions": {{
    "target": "ES2022",
    "lib": [{libs}],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "noEmit": true,
    "allowImportingTsExtensions": true,
    "esModuleInterop": true,
    "types": ["node"],
    "typeRoots": ["{rel_root}/node_modules/@types"],
    "baseUrl": ".",
    "paths": {{
      {generated_paths}
      "@/*": ["{rel_root}/typescript/testing/*"],
      {runtime_paths}
    }}
  }},
  "files": ["{test_script}"],
  "include": [{include}]
}}
"#
    );
    let filename = if type_check {
        "tsconfig.check.json"
    } else {
        "tsconfig.json"
    };
    let path = out_dir.join(filename);
    std::fs::write(&path, contents).expect("failed to write TypeScript config");
    path
}

fn tsconfig_runtimes(flavor: Flavor, rel_root: &Utf8PathBuf, type_check: bool) -> String {
    let mut runtime_paths = vec![
        format!(r#""@ubjs/core": ["{rel_root}/typescript/src/index"]"#),
        // Defensive: kept so mid-rollout fixtures still resolve. Drop
        // once all generated fixtures are regenerated to import @ubjs/core.
        format!(r#""uniffi-bindgen-react-native": ["{rel_root}/typescript/src/index"]"#),
    ];
    if flavor == Flavor::Napi {
        let entrypoint = if type_check { "" } else { "/lib" };
        runtime_paths.push(format!(
            r#""@ubjs/node": ["{rel_root}/runtimes/napi{entrypoint}"]"#
        ));
    }
    if flavor == Flavor::Wasm2 {
        // `paths` bypasses the package `exports` map, so the bare specifier
        // the generated index imports needs pointing at the node build.
        runtime_paths.push(format!(
            r#""@ubjs/wasm": ["{rel_root}/runtimes/wasm/node/src/index"]"#
        ));
        runtime_paths.push(format!(
            r#""@ubjs/wasm/core": ["{rel_root}/runtimes/wasm/core/src/index"]"#
        ));
        runtime_paths.push(format!(
            r#""@ubjs/wasm/browser": ["{rel_root}/runtimes/wasm/browser/src/index"]"#
        ));
        runtime_paths.push(format!(
            r#""@ubjs/wasm/node": ["{rel_root}/runtimes/wasm/node/src/index"]"#
        ));
    }
    runtime_paths.join(",\n      ")
}

/// Run a test script with tsx and experimental WASM module support.
pub(crate) fn run_tsx(test_script: &camino::Utf8Path, tsconfig: &Utf8Path) {
    let tsx = paths::node_modules_bin().join("tsx");
    run_cmd(
        command(&tsx)
            .arg("--tsconfig")
            .arg(tsconfig)
            .arg("--experimental-wasm-modules")
            .arg(test_script.as_str()),
    );
}

/// As [`run_tsx`], but evaluates `preload` first. Node runs `--import` modules
/// to completion — including their top-level `await` — before the entry
/// module, which is what lets the preload finish opening the wasm before the
/// test script imports anything that needs it.
pub(crate) fn run_tsx_with_preload(
    test_script: &camino::Utf8Path,
    preload: &camino::Utf8Path,
    tsconfig: &Utf8Path,
) {
    let tsx = paths::node_modules_bin().join("tsx");
    run_cmd(
        command(&tsx)
            .arg("--tsconfig")
            .arg(tsconfig)
            .arg("--experimental-wasm-modules")
            .arg("--import")
            .arg(format!("file://{preload}"))
            .arg(test_script.as_str()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires bootstrap yarn; run with --ignored after installing JS dependencies"]
    fn strict_tsx_gate_checks_only_selected_sources() {
        let out = paths::repo_root().join(format!(
            "target/fixture-typecheck-smoke-{}",
            std::process::id()
        ));
        struct Cleanup(Utf8PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(out.clone());
        std::fs::create_dir_all(&out).unwrap();
        let script = out.join("test.ts");
        let generated = out.join("generated");
        std::fs::create_dir_all(&generated).unwrap();
        // A sibling flavor must not contaminate this check.
        std::fs::write(
            out.join("other-flavor.ts"),
            "export const value: string = 1;",
        )
        .unwrap();
        std::fs::write(&script, "export const value: string = 'ok';").unwrap();
        let config = out.join("configs");
        for flavor in [Flavor::Napi, Flavor::Wasm, Flavor::Wasm2] {
            prepare_tsx(
                &script,
                &config.join(flavor.as_str()),
                Some(&generated),
                flavor,
            );
        }
        std::fs::write(&script, "document.createElement('div');").unwrap();
        assert!(std::panic::catch_unwind(|| {
            prepare_tsx(&script, &config, Some(&generated), Flavor::Napi)
        })
        .is_err());
        std::fs::write(&script, "export const value: string = undefined;").unwrap();
        assert!(std::panic::catch_unwind(|| {
            prepare_tsx(&script, &config, Some(&generated), Flavor::Wasm)
        })
        .is_err());
        std::fs::write(&script, "export const value: string = 'ok';").unwrap();
        // Include generated files even if no test imports them.
        std::fs::write(
            generated.join("unused.ts"),
            "export const value: string = 1;",
        )
        .unwrap();
        assert!(std::panic::catch_unwind(|| {
            prepare_tsx(&script, &config, Some(&generated), Flavor::Wasm)
        })
        .is_err());
    }
}
