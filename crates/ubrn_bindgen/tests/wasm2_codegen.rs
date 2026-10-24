/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Checks that `wrapper-ffi-player.ts` renders its Wasm2 branches: the
//! runtime import comes from `@ubjs/wasm/core`, the module exports
//! `PLAYER_DEFINITIONS` and `setNativeModule`, and the napi-only
//! `UniffiNativeModule.open(...)` path is absent.

#![cfg(feature = "wasm")]

use ubrn_bindgen::{render_player_lowlevel_for_test, AbiFlavor};

#[test]
fn wasm2_player_ffi_renders_expected_markers() {
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Wasm2, false)
        .expect("rendering wasm2 player wrapper-ffi.ts should succeed");

    // Wasm2 imports from the wasm runtime, not napi.
    assert!(
        rendered.contains(r#"import { FfiType, type ModuleDefinitions } from "@ubjs/wasm/core""#),
        "expected wasm2 runtime import in rendered output:\n{rendered}"
    );

    // `as const` would freeze the arrays, which then will not assign.
    assert!(
        rendered.contains("} satisfies ModuleDefinitions;"),
        "expected DEFINITIONS to be checked with `satisfies`:\n{rendered}"
    );
    assert!(
        !rendered.contains("} as const;"),
        "wasm2 DEFINITIONS must not be frozen; it has to assign to ModuleDefinitions:\n{rendered}"
    );

    // Wasm2 exposes the player definitions for late binding by the runtime.
    assert!(
        rendered.contains("export const PLAYER_DEFINITIONS = DEFINITIONS;"),
        "expected PLAYER_DEFINITIONS export in rendered output:\n{rendered}"
    );

    // Wasm2 lets the runtime push the native module in via setNativeModule.
    assert!(
        rendered.contains("export function setNativeModule"),
        "expected setNativeModule export in rendered output:\n{rendered}"
    );

    // The napi getter path (which calls UniffiNativeModule.open) must not
    // appear in the wasm2 output.
    assert!(
        !rendered.contains("UniffiNativeModule.open"),
        "wasm2 rendering must not include the napi UniffiNativeModule.open call:\n{rendered}"
    );

    // The wrapper must stay environment-neutral: opening the `.wasm` belongs to
    // the generated entrypoint, so importing an environment-specific subpath
    // here would make every browser and React Native bundle pull in node
    // built-ins.
    assert!(
        !rendered.contains("@ubjs/wasm/node") && !rendered.contains("@ubjs/wasm/browser"),
        "wasm2 wrapper must import only /core, not an environment subpath:\n{rendered}"
    );
    assert!(
        rendered.contains(r#"from "@ubjs/wasm/core""#),
        "expected the neutral /core import in wasm2 rendering:\n{rendered}"
    );
    assert!(
        !rendered.contains("openWasm"),
        "wasm2 wrapper must not open the wasm itself:\n{rendered}"
    );
    // Uninitialised use should say what to do rather than dereference undefined.
    assert!(
        rendered.contains("wasm module not initialised") && rendered.contains("uniffiInitAsync"),
        "expected an actionable not-initialised error in wasm2 rendering:\n{rendered}"
    );
}

#[test]
fn napi_player_ffi_still_uses_native_module_open() {
    // Sanity check that the non-Wasm2 player flavor still goes through the
    // napi UniffiNativeModule.open(...) path. This guards against the wasm2
    // branch accidentally being taken for non-wasm2 flavors.
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Napi, false)
        .expect("rendering napi player wrapper-ffi.ts should succeed");

    assert!(
        rendered.contains("UniffiNativeModule.open"),
        "expected napi flavor to use UniffiNativeModule.open:\n{rendered}"
    );
    assert!(
        !rendered.contains(r#"import { FfiType } from "@ubjs/wasm/core""#),
        "napi flavor must not import from @ubjs/wasm/core:\n{rendered}"
    );
    assert!(
        !rendered.contains("export const PLAYER_DEFINITIONS"),
        "napi flavor must not export PLAYER_DEFINITIONS:\n{rendered}"
    );

    // Wasm2-only: napi must not import from a runtime it does not depend on.
    assert!(
        rendered.contains("};"),
        "napi flavor should close DEFINITIONS plainly:\n{rendered}"
    );
    assert!(
        !rendered.contains("ModuleDefinitions"),
        "napi flavor must not reference the wasm runtime's ModuleDefinitions:\n{rendered}"
    );
}

#[test]
fn wasm2_player_ffi_promises_returns_under_async_delivery() {
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Wasm2, true)
        .expect("rendering wasm2 player wrapper-ffi.ts should succeed");
    assert!(
        rendered.contains("ubrn_uniffi_test_fn_func_add(lhs: number): Promise<number>;"),
        "expected the interface's functions to return promises:\n{rendered}"
    );
    // The rustbuffer pair is answered by the runtime, not the player, so it
    // stays synchronous.
    assert!(
        rendered.contains("rustbuffer_alloc(n: number): Uint8Array;"),
        "alloc stays synchronous under async delivery:\n{rendered}"
    );
    assert!(
        rendered.contains("every function below returns a Promise"),
        "expected the async-delivery note:\n{rendered}"
    );

    let sync = render_player_lowlevel_for_test(&AbiFlavor::Wasm2, false).unwrap();
    assert!(
        sync.contains("ubrn_uniffi_test_fn_func_add(lhs: number): number;"),
        "without async delivery the same function returns plainly:\n{sync}"
    );
    // The async entry's Promise comes from the player's poll loop, not from
    // delivery, so it is the only one.
    assert_eq!(
        sync.matches("Promise<").count(),
        1,
        "only the async entry returns a promise without async delivery:\n{sync}"
    );
    assert!(!sync.contains("returns a Promise"));
}

#[test]
fn wasm2_player_table_renders_an_async_entry() {
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Wasm2, false)
        .expect("rendering wasm2 player wrapper-ffi.ts should succeed");
    assert!(
        rendered.contains(
            r#"async: { poll: "ubrn_test_poll", cancel: "ubrn_test_cancel", complete: "ubrn_test_complete", free: "ubrn_test_free", ret: FfiType.UInt32 },"#
        ),
        "expected the table entry to name its future exports:\n{rendered}"
    );
    let entry = "ubrn_uniffi_test_fn_func_sleep(ms: number, uniffi_out_err: UniffiRustCallStatus, uniffi_signal: AbortSignal | undefined): Promise<number>;";
    assert!(
        rendered.contains(entry),
        "expected the async entry to return a promise of the completed value:\n{rendered}"
    );

    // Async delivery already promises every return; the entry is not wrapped twice.
    let delivered = render_player_lowlevel_for_test(&AbiFlavor::Wasm2, true).unwrap();
    assert!(
        delivered.contains(entry),
        "expected the same entry under async delivery:\n{delivered}"
    );
    assert!(
        !delivered.contains("Promise<Promise<"),
        "the async entry must not be wrapped in two promises:\n{delivered}"
    );
}
