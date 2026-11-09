/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! The player tag vocabulary exists in five places. This pins them to each other.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate is at <workspace>/crates/ubrn_bindgen")
        .to_path_buf()
}

/// Tag names read from the `FfiType` object literal `opener` introduces, in
/// both the `Name: {...}` and `Name: (args) => ...` forms. `opener` differs per
/// projection: a TS module exports the table, `lib.js` nests it in
/// `module.exports`.
fn js_ffi_type_keys(rel: &str, opener: &str) -> BTreeSet<String> {
    let src = std::fs::read_to_string(workspace_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"));
    let body = src
        .split_once(opener)
        .unwrap_or_else(|| panic!("no `{opener}` in {rel}"))
        .1;
    let keys: BTreeSet<String> = body
        .lines()
        .take_while(|l| !l.trim_start().starts_with('}'))
        .filter_map(|l| l.trim().split_once(':'))
        .map(|(k, _)| k.trim().to_owned())
        .filter(|k| k.chars().next().is_some_and(char::is_uppercase))
        .collect();
    assert!(!keys.is_empty(), "parsed no tag names out of {rel}");
    keys
}

/// Tag names read from the shim's `static constexpr Entry kTable[] = { ... }`
/// array: each entry is `{"Name", UBRN_TY_X},`.
fn shim_tag_table_keys() -> BTreeSet<String> {
    let rel = "runtimes/jsi/cpp/value_conv.h";
    let src = std::fs::read_to_string(workspace_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"));
    let table = src
        .split_once("static constexpr Entry kTable[] = {")
        .unwrap_or_else(|| panic!("no `static constexpr Entry kTable[] = {{` in {rel}"))
        .1
        .split_once("};")
        .unwrap_or_else(|| panic!("no closing `}}` for kTable in {rel}"))
        .0;
    table
        .split('"')
        .skip(1)
        .step_by(2)
        .map(|s| s.to_owned())
        .collect()
}

#[test]
fn every_projection_of_the_tag_vocabulary_agrees() {
    let core: BTreeSet<String> = uniffi_runtime_core::ALL_TAG_NAMES
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    const TS_OPENER: &str = "export const FfiType = {";
    // @ubjs/core owns the table; @ubjs/wasm/core re-exports it.
    let ts_core = js_ffi_type_keys("typescript/src/ffi-definitions.ts", TS_OPENER);
    // The table the napi-flavour generated code imports at runtime, so a tag
    // missing here is a `tag: undefined` at registration, not a build failure.
    let node = js_ffi_type_keys("runtimes/napi/lib.js", "FfiType: {");

    assert_eq!(
        core, ts_core,
        "core FfiTypeDesc vs typescript/src/ffi-definitions.ts"
    );
    assert_eq!(core, node, "core FfiTypeDesc vs runtimes/napi/lib.js");

    // The three tags with no wire representation never reach the shim's table.
    let wire_only: BTreeSet<String> = core
        .difference(
            &["ForeignBytes", "VoidPointer", "MutReference"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        )
        .cloned()
        .collect();
    let shim = shim_tag_table_keys();
    assert_eq!(
        wire_only, shim,
        "core wire-eligible names vs runtimes/jsi/cpp/value_conv.h kTable"
    );
}

// The emitter itself (`ffi_type_to_player` in
// crates/ubrn_bindgen/src/bindings/gen_typescript/ffi_module_player/type_mapping.rs)
// is pinned against `uniffi_runtime_core::ALL_TAG_NAMES` by a unit test in
// that same file: its match has no wildcard arm, so a new `general::FfiType`
// variant fails to compile there before this file is ever reached.
