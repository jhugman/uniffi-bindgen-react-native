/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    "tests/bindings/test_ext_types.ts" => [Jsi, Wasm, Napi, Wasm2, Jsi2],
    // Neither Hermes flavor: this variant imports `@/generated` as a bare
    // directory and uses top-level await, which need a tsc module/resolution
    // config that only the Napi and Wasm2 paths set up. Enabling Jsi2 fails at
    // typecheck with TS2307 + TS1378 before any player code runs, and Jsi is
    // out for the same reason — nothing here is specific to the player.
    "tests/bindings/test_ext_types_with_index.ts" => [Napi, Wasm2],
}
