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
//! `rename`/`exclude` takes effect in the generated TypeScript.
//!
//! Unlike the fixture harness, this drives the compiled CLI directly and
//! asserts on generated text, so it needs no Node/napi runtime and no
//! flavour bootstrap.

use std::fs;
use std::process::Command;
use std::sync::OnceLock;

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

/// Build the fixture cdylib once per test binary; the tests would otherwise
/// spawn `cargo build` repeatedly and serialise on cargo's build lock.
fn ensure_fixture_built() {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| build_fixture(ENUM_TYPES_PKG));
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

/// Run `... generate jsi bindings` (the only flavour that emits the native
/// `bless_pointer` call), returning `(api ts, ffi ts, cpp)`.
fn generate_jsi(
    config: &Utf8Path,
    ts_dir: &Utf8Path,
    cpp_dir: &Utf8Path,
) -> (String, String, String) {
    let lib = shared_lib_path(ENUM_TYPES_LIB);
    let status = Command::new(env!("CARGO_BIN_EXE_uniffi-bindgen-react-native"))
        .current_dir(repo_root())
        .arg("generate")
        .arg("jsi")
        .arg("bindings")
        .arg("--ts-dir")
        .arg(ts_dir)
        .arg("--cpp-dir")
        .arg(cpp_dir)
        .arg("--lib-file")
        .arg(&lib)
        .arg("--config")
        .arg(config)
        .arg(&lib)
        .status()
        .expect("failed to launch uniffi-bindgen-react-native");
    assert!(status.success(), "generate jsi bindings failed");

    (
        fs::read_to_string(ts_dir.join("enum_types.ts")).expect("read generated enum_types.ts"),
        fs::read_to_string(ts_dir.join("enum_types-ffi.ts"))
            .expect("read generated enum_types-ffi.ts"),
        fs::read_to_string(cpp_dir.join("enum_types.cpp")).expect("read generated enum_types.cpp"),
    )
}

/// Whether `haystack` contains `needle` as a whole identifier, so that a
/// renamed `RenamedIntList` does not count as a bare `IntList`.
fn contains_ident(haystack: &str, needle: &str) -> bool {
    haystack
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|word| word == needle)
}

/// A 0.32 global config file: `[crates.<namespace>]` must be merged into that
/// namespace's config. This is the branch `load_global_config` takes when the
/// file carries `crate-roots`/`defaults`/`crates`.
#[test]
fn global_config_crates_table_reaches_namespace() {
    ensure_fixture_built();
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
    ensure_fixture_built();
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
    ensure_fixture_built();
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
    ensure_fixture_built();
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

/// `IntList::Cons` holds `Box<IntList>`: the Box reference has to be renamed
/// too, not only the `IntList` definition, or `RenamedIntList`'s `Cons` variant
/// would still be typed as the now-nonexistent `IntList`.
#[test]
fn rename_reaches_type_inside_box() {
    ensure_fixture_built();
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

/// The native `bless_pointer` symbol is built from the ComponentInterface's
/// pre-rename name, so the TypeScript and C++ sides must spell it identically
/// even when the configured rename changes the user-facing type name.
#[test]
fn renamed_object_keeps_original_bless_pointer_symbol() {
    ensure_fixture_built();
    let scratch = Scratch::new("bless-rename");
    let config = scratch.file(
        "global.toml",
        r#"
[defaults.bindings.typescript]
rename = { "AnimalObject" = "RenamedAnimal" }
"#,
    );
    let (api_ts, ffi_ts, cpp) =
        generate_jsi(&config, &scratch.subdir("ts"), &scratch.subdir("cpp"));

    let original = "uniffi_internal_fn_method_animalobject_ffi__bless_pointer";
    let renamed = "uniffi_internal_fn_method_renamedanimal_ffi__bless_pointer";
    for (what, out) in [("api ts", &api_ts), ("ffi ts", &ffi_ts), ("cpp", &cpp)] {
        assert!(out.contains(original), "{what} is missing {original}");
        assert!(!out.contains(renamed), "{what} still spells {renamed}");
    }
    assert!(api_ts.contains("RenamedAnimal"), "rename did not reach TS");
}
