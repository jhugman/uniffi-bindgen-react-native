/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    // Jsi2: uses only supported shapes, but impractical to validate locally
    // (full Hermes run is >30 min wall-clock; the Jsi oracle also doesn't
    // complete cleanly locally). The `performance` polyfill that previously
    // blocked it is fixed (stale test-runner binary; rebuild picks it up).
    // Verify in CI.
    "tests/bindings/test_benchmark.ts" => [Jsi, Wasm, Napi, Wasm2, Jsi2],
}
