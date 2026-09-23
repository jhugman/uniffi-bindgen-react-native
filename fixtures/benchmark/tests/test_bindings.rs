/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    // Slow but not prohibitive: ~165s for all five flavors with
    // UBRN_PROFILE=release. A debug Hermes runs it ~37x slower, which is where
    // this fixture's reputation for taking forever came from — check
    // build/hermes/CMakeCache.txt for CMAKE_BUILD_TYPE before concluding it
    // has hung.
    "tests/bindings/test_benchmark.ts" => [Jsi, Wasm, Napi, Wasm2, Jsi2],
}
