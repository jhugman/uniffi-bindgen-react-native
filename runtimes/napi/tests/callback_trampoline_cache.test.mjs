/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Passing a callback as an FFI argument must not rebuild its trampoline every call.
//
// Building one costs a leaked `napi_ref` to the function, a leaked userdata box, a
// fresh JS function, a fresh ThreadsafeFunction (plus an entry on the env's TSFN
// registry), a leaked TrampolineUserdata, and a libffi closure holding an
// executable page. None of it is ever freed — deliberately, because the library
// may invoke a trampoline from any thread at any later time — so building one per
// call leaks ~3 KB per call. Measured against the generated API before this cache,
// every `await` of an async uniffi function leaked ~3.2 KB, because the poll loop
// passes its continuation callback on every poll.
//
// The assertion is deliberately NOT an RSS measurement. V8 moves its heap
// reservation by megabytes, which makes per-call byte accounting at this scale
// noisy enough to be useless (a 16 KB payload loop measured 0.9 MB, 9.0 MB and
// 0.1 MB of "growth" at increasing iteration counts). Instead we assert the
// invariant that actually matters and is exact: the same function object reuses
// the same trampoline, and a different function object gets its own.
import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

const SYMBOLS = {
  rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
  rustbuffer_free: "uniffi_test_rustbuffer_free",
  rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
};

function openModule() {
  return UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      simple_cb: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_call_callback: {
        args: [FfiType.Callback("simple_cb"), FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });
}

// The runtime records a cached trampoline on the function object under a hidden
// per-registration Symbol, the same way it records RustBuffer ownership on a view.
// Reading it back is white-box, but it is the only exact signal available: the
// trampoline pointer is not otherwise observable from JS.
function trampolineMarkers(fn) {
  return Object.getOwnPropertySymbols(fn).map((s) => fn[s]);
}

test("the same callback function reuses one trampoline across calls", () => {
  const nm = openModule();
  let hits = 0;
  const cb = (_handle, value) => {
    hits += value;
  };

  const status = { code: 0 };
  nm.uniffi_test_fn_call_callback(cb, 1n, 1, status);
  assert.strictEqual(status.code, 0);

  const afterFirst = trampolineMarkers(cb);
  assert.strictEqual(
    afterFirst.length,
    1,
    "expected exactly one cached trampoline marker after the first call",
  );
  assert.ok(afterFirst[0], "cached trampoline pointer must be non-zero");

  for (let i = 0; i < 50; i++) {
    nm.uniffi_test_fn_call_callback(cb, 1n, 1, status);
    assert.strictEqual(status.code, 0);
  }

  assert.strictEqual(hits, 51, "the callback must still be invoked every call");
  assert.deepStrictEqual(
    trampolineMarkers(cb),
    afterFirst,
    "the trampoline must be reused, not rebuilt, on later calls",
  );
});

test("distinct callback functions get distinct trampolines", () => {
  const nm = openModule();
  const status = { code: 0 };
  const a = () => {};
  const b = () => {};

  nm.uniffi_test_fn_call_callback(a, 1n, 1, status);
  nm.uniffi_test_fn_call_callback(b, 1n, 1, status);

  const [ptrA] = trampolineMarkers(a);
  const [ptrB] = trampolineMarkers(b);
  assert.ok(ptrA && ptrB, "both functions must carry a cached trampoline");
  assert.notStrictEqual(
    ptrA,
    ptrB,
    "two different JS functions must not share a trampoline",
  );
});

test("a cached callback still dispatches correctly after many calls", () => {
  const nm = openModule();
  const seen = [];
  const cb = (handle, value) => {
    seen.push([handle, value]);
  };

  const status = { code: 0 };
  for (let i = 0; i < 5; i++) {
    nm.uniffi_test_fn_call_callback(cb, BigInt(i), i, status);
    assert.strictEqual(status.code, 0);
  }

  assert.deepStrictEqual(
    seen,
    [
      [0n, 0],
      [1n, 1],
      [2n, 2],
      [3n, 3],
      [4n, 4],
    ],
    "arguments must be marshalled correctly on every call, cached or not",
  );
});
