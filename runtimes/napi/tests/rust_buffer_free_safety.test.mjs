/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// `rustbuffer_free` must free only memory the library actually owns.
//
// The ownership marker is the sole authority: absent means "not ours, refuse"; 0 means
// "ours, nothing left to free"; > 0 is the true allocation capacity. Guessing a capacity
// for an unmarked view — from `byteLength`, say — would pass V8-owned memory to Rust's
// allocator and corrupt the heap.
//
// That authority only holds if every view the runtime hands out is marked, so these also
// pin the two cases where the capacity carries no information on its own:
// `rustbuffer_alloc(0)`, and lift handoffs where `capacity == byteLength`.
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
    callbacks: {},
    functions: {
      uniffi_test_fn_make_buffer: {
        args: [FfiType.UInt8, FfiType.UInt32],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
      // Returns capacity = n with len = 0: spare capacity and no payload.
      uniffi_test_rustbuffer_alloc: {
        args: [FfiType.UInt64],
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
    },
  });
}

const live = (nm) => ({
  count: nm.uniffi_test_live_buffer_count({ code: 0 }),
  bytes: Number(nm.uniffi_test_live_buffer_bytes({ code: 0 })),
});

test("freeing a plain JS Uint8Array is refused, not passed to the allocator", () => {
  const nm = openModule();
  // A view the runtime never handed out carries no marker, so its capacity is unknown
  // and its memory is V8's. Refuse rather than guess.
  assert.throws(
    () => nm.rustbuffer_free(new Uint8Array(64)),
    /unowned Uint8Array/,
    "an unmarked view must be refused",
  );
  // A zero-length foreign view is refused too — it is still not ours.
  assert.throws(
    () => nm.rustbuffer_free(new Uint8Array(0)),
    /unowned Uint8Array/,
  );
});

test("a returned buffer whose capacity equals its length is still released", () => {
  // `capacity == byteLength` is the case where the marker looks redundant. It is not:
  // with an authoritative marker, skipping the write would make the view unfreeable.
  const nm = openModule();
  const before = live(nm);

  const status = { code: 0 };
  const view = nm.uniffi_test_fn_make_buffer(0x11, 32, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(view.byteLength, 32);
  nm.rustbuffer_free(view);

  assert.deepStrictEqual(
    live(nm),
    before,
    "the allocation must be fully released",
  );
});

test("a returned buffer with spare capacity and no payload releases that capacity", () => {
  // len == 0 with capacity > 0. A zero-length typed array cannot carry the data pointer
  // forward, so the handoff must release the allocation itself: by the time a caller
  // reaches `rustbuffer_free`, there is nothing left to work from.
  const nm = openModule();
  const before = live(nm);

  const status = { code: 0 };
  const view = nm.uniffi_test_rustbuffer_alloc(1024n, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(view.byteLength, 0);
  assert.deepStrictEqual(
    live(nm),
    before,
    "spare capacity must already be released",
  );

  // Marked JS-owned, so cleanup is a no-op however often it runs.
  nm.rustbuffer_free(view);
  nm.rustbuffer_free(view);
  assert.deepStrictEqual(live(nm), before);
});

test("rustbuffer_alloc(0) round-trips through free", () => {
  const nm = openModule();
  const before = live(nm);
  const view = nm.rustbuffer_alloc(0);
  assert.strictEqual(view.byteLength, 0);
  nm.rustbuffer_free(view);
  nm.rustbuffer_free(view);
  assert.deepStrictEqual(live(nm), before);
});

test("repeated frees of a live returned buffer do not double free", () => {
  const nm = openModule();
  const before = live(nm);

  const status = { code: 0 };
  const view = nm.uniffi_test_fn_make_buffer(0x22, 128, status);
  assert.strictEqual(status.code, 0);

  nm.rustbuffer_free(view);
  assert.deepStrictEqual(live(nm), before);
  // The marker is reset on release, so the second call is a no-op rather than a
  // double free of the same pointer.
  nm.rustbuffer_free(view);
  nm.rustbuffer_free(view);
  assert.deepStrictEqual(live(nm), before);
});
