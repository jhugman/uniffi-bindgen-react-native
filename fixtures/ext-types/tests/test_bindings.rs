/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    "tests/bindings/test_ext_types.ts" => [Jsi, Wasm, Napi, Wasm2, Channel],
    // No Channel: exercises the generated `uniffiInitAsync()` singleton
    // directly, which the channel test harness bypasses (it wires ports by
    // hand, standing in for a codegen delivery mode that doesn't exist yet).
    "tests/bindings/test_ext_types_with_index.ts" => [Napi, Wasm2],
}
