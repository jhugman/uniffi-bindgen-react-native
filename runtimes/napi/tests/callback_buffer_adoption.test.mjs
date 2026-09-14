/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// A buffer a callback returns must be adopted, not copied.
//
// Generated callback code lowers a non-string return value through
// `rustbuffer_alloc`, which hands back a library-owned buffer, and then passes that
// view out as the callback's result. Nothing frees a lowered value — codegen frees
// returned buffers, never lowered ones — so if the runtime *copies* the view into a
// second allocation, the original is orphaned and every invocation leaks one whole
// serialized payload. That was #432's documented remaining limitation: no capacity
// marker was reachable from the callback path, so it always copied.
//
// The marshalling path now carries the registration, so it adopts exactly what is
// adoptable: a marked (library-owned) view is handed straight to the callee, an
// ordinary V8 array is still copied.
//
// These assertions use the fixture's exact live-allocation counters rather than RSS.
// That matters here beyond the usual noise argument: much of the RSS growth around
// external ArrayBuffers is reclaimed by V8 weak callbacks that only run once the
// event loop turns, so an RSS measurement in a tight loop conflates a genuine
// never-freed allocation with memory that is merely pending. A counter cannot be
// fooled either way.
import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

function openModule() {
  return UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register({
    symbols: {
      rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
      rustbuffer_free: "uniffi_test_rustbuffer_free",
      rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
    },
    structs: {},
    callbacks: {
      buffer_returning_cb: {
        args: [FfiType.UInt64],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_call_buffer_returning_callback: {
        args: [FfiType.Callback("buffer_returning_cb"), FfiType.UInt64],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
      uniffi_test_live_buffer_count: {
        args: [],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
      uniffi_test_live_buffer_bytes: {
        args: [],
        ret: FfiType.Int64,
        hasRustCallStatus: true,
      },
    },
  });
}

const live = (nm) => ({
  count: nm.uniffi_test_live_buffer_count({ code: 0 }),
  bytes: Number(nm.uniffi_test_live_buffer_bytes({ code: 0 })),
});

const PAYLOAD = 1024;
const ITERATIONS = 64;

test("a library-owned buffer returned from a callback is not orphaned", () => {
  const nm = openModule();
  const before = live(nm);

  // Exactly what generated callback code does: lower the return value through
  // `rustbuffer_alloc`, fill it, and hand the view back as the result.
  const cb = (_handle) => {
    const view = nm.rustbuffer_alloc(PAYLOAD);
    view.fill(0x5a);
    return view;
  };

  const status = { code: 0 };
  for (let i = 0; i < ITERATIONS; i++) {
    const len = nm.uniffi_test_fn_call_buffer_returning_callback(
      cb,
      1n,
      status,
    );
    assert.strictEqual(status.code, 0);
    assert.strictEqual(len, PAYLOAD, "Rust must see the whole payload");
  }

  const after = live(nm);
  assert.strictEqual(
    after.count,
    before.count,
    `orphaned ${after.count - before.count} buffers over ${ITERATIONS} callback returns`,
  );
  assert.strictEqual(
    after.bytes,
    before.bytes,
    `orphaned ${after.bytes - before.bytes} bytes over ${ITERATIONS} callback returns`,
  );
});

test("adoption is observable: the returned view's marker is cleared", () => {
  const nm = openModule();
  let captured;
  const cb = (_handle) => {
    captured = nm.rustbuffer_alloc(PAYLOAD);
    captured.fill(1);
    return captured;
  };

  const status = { code: 0 };
  nm.uniffi_test_fn_call_buffer_returning_callback(cb, 1n, status);
  assert.strictEqual(status.code, 0);

  // A marker of 0 means the allocation was handed to the callee, so there is nothing
  // left for us to free. Had the runtime copied instead, the marker would still hold
  // the capacity and the original allocation would be unreachable.
  const [marker] = Object.getOwnPropertySymbols(captured);
  assert.ok(marker, "the lowered view should carry an ownership marker");
  assert.strictEqual(captured[marker], 0n, "the view should have been adopted");
});

test("a plain JS Uint8Array returned from a callback is still copied", () => {
  // Not ours to give away: V8 memory must never reach the library's allocator, so
  // this path must keep copying. The copy is what the callee frees, so the books
  // still balance.
  const nm = openModule();
  const before = live(nm);

  const cb = (_handle) => {
    const arr = new Uint8Array(PAYLOAD);
    arr.fill(0x33);
    return arr;
  };

  const status = { code: 0 };
  for (let i = 0; i < ITERATIONS; i++) {
    const len = nm.uniffi_test_fn_call_buffer_returning_callback(
      cb,
      1n,
      status,
    );
    assert.strictEqual(status.code, 0);
    assert.strictEqual(len, PAYLOAD);
  }

  assert.deepStrictEqual(live(nm), before, "the copies must all be released");
});
