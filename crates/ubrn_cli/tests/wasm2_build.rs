/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! End-to-end cover for `ubrn build wasm2`, the command a consuming project
//! runs.
//!
//! Not the `ubrn_cli_testing` recording harness, which asserts the commands a
//! build would issue without running them. What can go wrong here is files
//! that do not line up — a `.wasm` left in `target/`, bindings naming
//! something nothing staged — and only a real run shows that.
use std::fs;
use std::process::Command;

use anyhow::Result;
use camino::Utf8PathBuf;

fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize_utf8()
        .expect("canonicalize repo root")
}

/// `#[ignore]` because this builds a fixture for `wasm32-unknown-unknown`,
/// and the default `cargo test` run — the one the unit-test job makes over
/// the whole workspace — has no reason to have that target installed. The
/// wasm2 job does, and asks for this test by name:
///
///     cargo test -p uniffi-bindgen-react-native --test wasm2_build -- --ignored
#[test]
#[ignore]
fn build_wasm2_lays_out_bindings_entrypoints_and_the_wasm() -> Result<()> {
    let root = repo_root();
    let crate_dir = root.join("fixtures/coverall");
    let project = Utf8PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ubrn-wasm2-build");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project)?;

    fs::write(
        project.join("package.json"),
        r#"{
  "name": "wasm2-build-test",
  "version": "0.1.0",
  "repository": { "type": "git", "url": "https://example.invalid/t.git" }
}
"#,
    )?;
    fs::write(
        project.join("ubrn.config.yaml"),
        format!(
            "rust:\n  directory: {crate_dir}\n  manifestPath: Cargo.toml\nwasm2:\n  ts: src/generated\n"
        ),
    )?;

    let out = Command::new(env!("CARGO_BIN_EXE_uniffi-bindgen-react-native"))
        .current_dir(&project)
        .args(["build", "wasm2", "--config", "./ubrn.config.yaml"])
        .output()?;
    assert!(
        out.status.success(),
        "ubrn build wasm2 failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // The bindings, both entrypoints, and the module staged beside them.
    for rel in [
        "src/generated/coverall.ts",
        "src/generated/coverall-ffi.ts",
        "src/generated/uniffi_coverall.wasm",
        "src/generated/index.ts",
    ] {
        assert!(project.join(rel).exists(), "expected {rel} to be generated");
    }

    // The name the bindings ask for has to be the name that was staged —
    // this is what the staging step exists to guarantee.
    let ffi = fs::read_to_string(project.join("src/generated/coverall-ffi.ts"))?;
    assert!(
        !ffi.contains("@ubjs/wasm/node") && !ffi.contains("@ubjs/wasm/browser"),
        "generated bindings must stay environment-neutral"
    );
    // The bindgen's index is the entrypoint: it opens the module, and picks
    // its environment through the runtime package's `exports` conditions
    // rather than through a per-environment file.
    let index = fs::read_to_string(project.join("src/generated/index.ts"))?;
    assert!(
        index.contains(r#"from "@ubjs/wasm""#) && index.contains("openWasm"),
        "the generated index should open the wasm:\n{index}"
    );
    assert!(
        index.contains("uniffiInitAsync(source: WasmSource)"),
        "the generated index should require the caller to name the asset:\n{index}"
    );
    assert!(
        index.contains("uniffi_coverall.wasm"),
        "the generated index should say which module was staged:\n{index}"
    );

    // Staging rewrites a copy, adding the growable table export the player
    // needs; cargo's own artifact is left as cargo wrote it.
    const TABLE_EXPORT: &[u8] = b"__indirect_function_table";
    let staged = fs::read(project.join("src/generated/uniffi_coverall.wasm"))?;
    assert!(
        staged
            .windows(TABLE_EXPORT.len())
            .any(|w| w == TABLE_EXPORT),
        "the staged wasm should carry the exported function table"
    );

    Ok(())
}
