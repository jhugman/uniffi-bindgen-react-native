/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    "tests/bindings/test_gc_callbacks_crasher.ts" => [Jsi, Wasm, Napi, Wasm2, Jsi2],
    "tests/bindings/test_gc_callbacks_crasher_async.ts" => [AsyncWasm, AsyncNapi, AsyncElectron],
    // Forces collections through the Hermes test-runner's `__hermesGc`.
    "tests/bindings/test_gc_releases_delegates.ts" => [Jsi, Jsi2],
}
