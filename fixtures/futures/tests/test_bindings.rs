/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
ubrn_macros::build_foreign_language_testcases! {
    // No Wasm2 (nor Wasm): the `TimerFuture` in `src/lib.rs` wakes itself with
    // `std::thread::spawn`, which single-threaded wasm32 cannot do. The panic
    // aborts mid-call holding a Rust mutex, and `freeFunc` then aborts again
    // trying to re-lock it. Re-enable once TimerFuture drops host threads.
    "tests/bindings/test_futures.ts" => [Jsi, Napi],
}
