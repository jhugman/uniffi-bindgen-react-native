/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    "tests/bindings/test_ext_types.ts" => [Jsi, Wasm, Napi, Wasm2, Jsi2],
    // This variant imports `@/generated` as a bare directory, which resolves
    // to the index.ts bindgen writes for the player flavors. Jsi is out: its
    // entrypoint is a turbo module, so there is no index.ts to import.
    "tests/bindings/test_ext_types_with_index.ts" => [Jsi2, Napi, Wasm2],
}
