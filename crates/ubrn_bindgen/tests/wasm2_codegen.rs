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
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Wasm2)
        .expect("rendering wasm2 player wrapper-ffi.ts should succeed");

    // Wasm2 imports FfiType from the wasm runtime, not the napi runtime.
    assert!(
        rendered.contains(r#"import { FfiType } from "@ubjs/wasm/core""#),
        "expected wasm2 runtime import in rendered output:\n{rendered}"
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
        rendered.contains(r#"import { FfiType } from "@ubjs/wasm/core""#),
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
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Napi)
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
}
