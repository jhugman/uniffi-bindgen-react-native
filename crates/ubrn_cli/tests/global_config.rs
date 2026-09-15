/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! End-to-end cover for `--config` on `generate <flavor> bindings`, which
//! `crates/ubrn_bindgen/src/cli.rs::load_global_config` interprets in two
//! shapes:
//!
//!  * a uniffi 0.32 *global* config file, with `[crate-roots]`, `[defaults]`
//!    and/or `[crates.<name>]` sections, and
//!  * a legacy *flat* file, which is treated as a per-crate override for
//!    every crate.
//!
//! Both must reach the namespace config that `run_typescript_pipeline`
//! republishes as `[bindings.react-native]`, so a `[bindings.typescript]`
//! `rename`/`exclude` takes effect in the generated TypeScript. The
//! `rename_reaches_type_inside_box` case additionally pins `BoxRenameFix`:
//! uniffi's rename pass has no `Type::Box` arm, so without the post-pass a
//! renamed `Box<T>` would still spell the old name inside the Box.
//!
//! Unlike the fixture harness, this drives the compiled CLI directly and
//! asserts on generated text, so it needs no Node/napi runtime and no
//! flavour bootstrap.

use std::fs;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

/// Fixture package and its cdylib name (see fixtures/enum-types/Cargo.toml).
const ENUM_TYPES_PKG: &str = "uniffi-fixture-enum-types";
const ENUM_TYPES_LIB: &str = "enum_types";

fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize_utf8()
        .expect("canonicalize repo root")
}

fn target_debug_dir() -> Utf8PathBuf {
    // Cargo sets CARGO_TARGET_TMPDIR to `<target>/tmp` for integration tests.
    Utf8PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("target tmpdir has a parent")
        .to_owned()
        .join("debug")
}

fn shared_lib_path(lib: &str) -> Utf8PathBuf {
    let (prefix, ext) = if cfg!(target_os = "windows") {
        ("", "dll")
    } else if cfg!(target_os = "macos") {
        ("lib", "dylib")
    } else {
        ("lib", "so")
    };
    target_debug_dir().join(format!("{prefix}{lib}.{ext}"))
}

fn build_fixture(pkg: &str) {
    let status = Command::new("cargo")
        .args(["build", "-p", pkg, "--lib"])
        .status()
        .expect("failed to launch cargo");
    assert!(status.success(), "cargo build -p {pkg} --lib failed");
}

/// A scratch directory under the target tmpdir, keyed by test name.
struct Scratch(Utf8PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Utf8PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("ubrn-global-config")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn file(&self, name: &str, contents: &str) -> Utf8PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).expect("write scratch file");
        path
    }

    fn subdir(&self, name: &str) -> Utf8PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(&path).expect("create scratch subdir");
        path
    }
}

/// Run `uniffi-bindgen-react-native generate napi bindings --config <config>`
/// against the enum-types cdylib, returning the generated `enum_types.ts`.
fn generate_napi(config: &Utf8Path, out: &Utf8Path) -> String {
    let status = Command::new(env!("CARGO_BIN_EXE_uniffi-bindgen-react-native"))
        .current_dir(repo_root())
        .arg("generate")
        .arg("napi")
        .arg("bindings")
        .arg("--lib-absolute")
        .arg("--library")
        .arg("--ts-dir")
        .arg(out)
        .arg("--config")
        .arg(config)
        .arg(shared_lib_path(ENUM_TYPES_LIB))
        .status()
        .expect("failed to launch uniffi-bindgen-react-native");
    assert!(status.success(), "generate napi bindings failed");

    fs::read_to_string(out.join("enum_types.ts")).expect("read generated enum_types.ts")
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Whether `haystack` contains `needle` as a whole identifier, so that a
/// renamed `RenamedIntList` does not count as a bare `IntList`.
fn contains_ident(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// A 0.32 global config file: `[crates.<namespace>]` must be merged into that
/// namespace's config. This is the branch `load_global_config` takes when the
/// file carries `crate-roots`/`defaults`/`crates`.
#[test]
fn global_config_crates_table_reaches_namespace() {
    build_fixture(ENUM_TYPES_PKG);
    let scratch = Scratch::new("crates-table");
    let config = scratch.file(
        "global.toml",
        r#"
[crates.enum_types.bindings.typescript]
rename = { "AnimalNoReprInt" = "CratesRenamedAnimal" }
exclude = ["AnimalLargeUInt"]
"#,
    );
    let ts = generate_napi(&config, &scratch.subdir("out"));

    assert!(
        ts.contains("CratesRenamedAnimal"),
        "[crates.<name>] rename did not reach the pipeline:\n{ts}"
    );
    assert!(
        !contains_ident(&ts, "AnimalNoReprInt"),
        "renamed type still present under its old name"
    );
    assert!(
        !contains_ident(&ts, "AnimalLargeUInt"),
        "[crates.<name>] exclude did not reach the pipeline"
    );
    // A neighbour that the config did not mention still works.
    assert!(
        contains_ident(&ts, "AnimalUInt") && contains_ident(&ts, "getAnimal"),
        "unrelated exports were dropped"
    );
}

/// A global config with only `[defaults]` applies to every crate.
#[test]
fn global_config_defaults_reach_namespace() {
    build_fixture(ENUM_TYPES_PKG);
    let scratch = Scratch::new("defaults");
    let config = scratch.file(
        "global.toml",
        r#"
[defaults.bindings.typescript]
rename = { "AnimalNoReprInt" = "DefaultsRenamedAnimal" }
"#,
    );
    let ts = generate_napi(&config, &scratch.subdir("out"));

    assert!(
        ts.contains("DefaultsRenamedAnimal"),
        "[defaults] rename did not reach the pipeline:\n{ts}"
    );
    assert!(
        !contains_ident(&ts, "AnimalNoReprInt"),
        "renamed type still present under its old name"
    );
}

/// A flat file (no `crate-roots`/`defaults`/`crates` keys) is the legacy shape:
/// it overrides *every* crate's config.
#[test]
fn flat_config_is_per_crate_override() {
    build_fixture(ENUM_TYPES_PKG);
    let scratch = Scratch::new("flat");
    let config = scratch.file(
        "flat.toml",
        r#"
[bindings.typescript]
rename = { "AnimalNoReprInt" = "FlatRenamedAnimal" }
exclude = ["AnimalLargeUInt"]
"#,
    );
    let ts = generate_napi(&config, &scratch.subdir("out"));

    assert!(
        ts.contains("FlatRenamedAnimal"),
        "flat config rename did not reach the pipeline:\n{ts}"
    );
    assert!(
        !contains_ident(&ts, "AnimalNoReprInt"),
        "renamed type still present under its old name"
    );
    assert!(
        !contains_ident(&ts, "AnimalLargeUInt"),
        "flat config exclude did not reach the pipeline"
    );
}

/// The rename table also keys record fields (`Type.field`) and methods
/// (`Type.method`), not just top-level names.
#[test]
fn rename_reaches_record_field_and_method() {
    build_fixture(ENUM_TYPES_PKG);
    let scratch = Scratch::new("field-method-rename");
    let config = scratch.file(
        "global.toml",
        r#"
[defaults.bindings.typescript]
rename = { "AnimalRecord.value" = "renamedValue", "AnimalObject.record" = "renamedRecord" }
"#,
    );
    let ts = generate_napi(&config, &scratch.subdir("out"));

    assert!(
        ts.contains("renamedValue: number"),
        "record field rename did not reach the pipeline:\n{ts}"
    );
    assert!(
        ts.contains("renamedRecord(): AnimalRecord"),
        "method rename did not reach the pipeline:\n{ts}"
    );
}

/// `IntList::Cons` holds `Box<IntList>`: the recursive Box reference has to be
/// renamed too, not only the `IntList` definition. `BoxRenameFix` exists for
/// exactly this; without it `RenamedIntList`'s `Cons` variant would still be
/// typed as the now-nonexistent `IntList`.
#[test]
fn rename_reaches_type_inside_box() {
    build_fixture(ENUM_TYPES_PKG);
    let scratch = Scratch::new("box-rename");
    let config = scratch.file(
        "global.toml",
        r#"
[defaults.bindings.typescript]
rename = { "IntList" = "RenamedIntList" }
"#,
    );
    let ts = generate_napi(&config, &scratch.subdir("out"));

    assert!(
        ts.contains("[number, RenamedIntList"),
        "the Box<IntList> element type was not renamed:\n{ts}"
    );
    assert!(
        !contains_ident(&ts, "IntList"),
        "a bare `IntList` reference survived the rename inside the Box"
    );
    assert!(
        contains_ident(&ts, "RenamedIntList"),
        "the renamed enum definition/uses are missing"
    );
}
