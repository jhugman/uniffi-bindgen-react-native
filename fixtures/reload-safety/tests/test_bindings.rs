/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    // Jsi2 only. Every one of these asserts on what survives a JS runtime being
    // destroyed while the native library stays loaded, which is a boundary only
    // the generic JSI player has: napi and wasm have no equivalent in their
    // harnesses, and jsi builds a per-library C++ bridge whose lifetime is not
    // the player's.
    "tests/bindings/test_reload_teardown.ts" => [Jsi2],
    "tests/bindings/test_reload_stale_callback.ts" => [Jsi2],
    "tests/bindings/test_reload_root_pinned.ts" => [Jsi2],
    "tests/bindings/test_reload_parked_worker.ts" => [Jsi2],
}
