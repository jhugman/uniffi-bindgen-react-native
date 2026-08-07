/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Ownership accounting for RustBuffers crossing the FFI boundary.
//
// These tests assert that every allocation the library makes is eventually released. They
// read the fixture's exact live-allocation counters rather than sampling RSS: V8 moves its
// own heap reservation by megabytes, which would swamp a few hundred leaked bytes per call
// and make the test both slow and flaky.
//
// The regression they guard is a leak in the *argument* lowering path. Codegen lowers a
// RustBuffer argument by calling `rustbuffer_alloc(n)`, filling the returned view in place,
// and passing that view as the argument. If the runtime converts that view by copying it into
// a second buffer, the first one is orphaned: codegen never frees a lowered argument, and the
// view carries a no-op finalizer. One whole payload then leaks per call.
import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

const LIB_PATH = libPath("uniffi_napi_test_lib");

const SYMBOLS = {
  rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
  rustbuffer_free: "uniffi_test_rustbuffer_free",
  rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
};

const FUNCTIONS = {
  uniffi_test_fn_echo_buffer: {
    args: [FfiType.RustBuffer],
    ret: FfiType.RustBuffer,
    hasRustCallStatus: true,
  },
  uniffi_test_fn_buffer_len: {
    args: [FfiType.RustBuffer],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  },
  uniffi_test_fn_concat_buffers: {
    args: [FfiType.RustBuffer, FfiType.RustBuffer],
    ret: FfiType.RustBuffer,
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
};

function openModule() {
  const nm = UniffiNativeModule.open(LIB_PATH).register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: FUNCTIONS,
  });
  const live = () => ({
    count: nm.uniffi_test_live_buffer_count({ code: 0 }),
    bytes: Number(nm.uniffi_test_live_buffer_bytes({ code: 0 })),
  });
  return { nm, live };
}

// Sized so a leak is unmistakable: 64 iterations x 1024 bytes is 64 KB, and the assertion is
// an exact equality rather than a threshold.
const ITERATIONS = 64;
const PAYLOAD_SIZE = 1024;

test("lowered argument buffer is released (rustbuffer_alloc view as argument)", () => {
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    // Exactly what codegen does: allocate through the library, fill in place, pass the view.
    const view = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    view.fill(i & 0xff);
    const status = { code: 0 };
    const result = nm.uniffi_test_fn_echo_buffer(view, status);
    assert.strictEqual(status.code, 0);
    assert.strictEqual(result.byteLength, PAYLOAD_SIZE);
    // Codegen's `finally` frees the returned buffer. It never frees the argument.
    nm.rustbuffer_free(result);
  }

  const after = live();
  assert.strictEqual(
    after.count,
    before.count,
    `leaked ${after.count - before.count} buffers over ${ITERATIONS} calls`,
  );
  assert.strictEqual(
    after.bytes,
    before.bytes,
    `leaked ${after.bytes - before.bytes} bytes over ${ITERATIONS} calls`,
  );
});

test("lowered argument buffer is released for a void-returning call", () => {
  // Isolates the argument direction: nothing comes back, so any growth must be the argument.
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    const view = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    const status = { code: 0 };
    const len = nm.uniffi_test_fn_buffer_len(view, status);
    assert.strictEqual(status.code, 0);
    // `rustbuffer_alloc` reports len 0; the library sees the capacity as the length.
    assert.strictEqual(typeof len, "number");
  }

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});

test("both lowered argument buffers are released for a two-argument call", () => {
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    const a = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    const b = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    const status = { code: 0 };
    const result = nm.uniffi_test_fn_concat_buffers(a, b, status);
    assert.strictEqual(status.code, 0);
    nm.rustbuffer_free(result);
  }

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});

test("plain JS Uint8Array argument is copied, not adopted", () => {
  // A V8-owned array is not the library's to take. The runtime must copy it, and the copy
  // must be released. Adopting it would hand the library a pointer into the V8 heap.
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    const input = new Uint8Array(PAYLOAD_SIZE);
    input.fill(i & 0xff);
    const status = { code: 0 };
    const result = nm.uniffi_test_fn_echo_buffer(input, status);
    assert.strictEqual(status.code, 0);
    // The echo must still be correct, which it would not be if the bytes were not copied.
    assert.strictEqual(result[0], i & 0xff);
    assert.strictEqual(result[PAYLOAD_SIZE - 1], i & 0xff);
    nm.rustbuffer_free(result);
  }

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});

test("alloc/free round-trip without an FFI call is balanced", () => {
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    const view = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    nm.rustbuffer_free(view);
  }

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});

test("freeing an already-adopted argument view does not double free", () => {
  // Defensive: codegen does not free lowered arguments today, but a caller that does must not
  // corrupt the heap. Once the library has adopted the view, the second release is a no-op.
  const { nm, live } = openModule();
  const before = live();

  for (let i = 0; i < ITERATIONS; i++) {
    const view = nm.rustbuffer_alloc(PAYLOAD_SIZE);
    const status = { code: 0 };
    const result = nm.uniffi_test_fn_echo_buffer(view, status);
    assert.strictEqual(status.code, 0);
    nm.rustbuffer_free(view);
    nm.rustbuffer_free(result);
  }

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});

test("empty rustbuffer_alloc view is accounted for", () => {
  const { nm, live } = openModule();
  const before = live();

  const view = nm.rustbuffer_alloc(0);
  assert.strictEqual(view.byteLength, 0);
  nm.rustbuffer_free(view);

  const after = live();
  assert.strictEqual(after.count, before.count);
  assert.strictEqual(after.bytes, before.bytes);
});
