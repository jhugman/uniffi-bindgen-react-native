/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { UniffiNativeModule } from "../src/module.js";

/**
 * Pre-assembled bytes for a synthetic future-host wasm module.
 *
 * Shape:
 *
 *   (module
 *     (memory (export "memory") 1)
 *     (table (export "__indirect_function_table") 4 funcref)
 *     (type $cont (func (param i64 i32))) ;; (data, code) -> ()
 *
 *     (func (export "__ubrn_alloc") ...)
 *     (func (export "__ubrn_free") ...)
 *
 *     ;; copies a pre-baked continuation trampoline (shapeId=0,
 *     ;; params=[i64, i32], no return) from a data segment into the
 *     ;; caller's buffer.
 *     (func (export "__ubrn_emit_trampoline") ...)
 *
 *     ;; uniffi_poll(handle: i64, cb: i32, data: i64):
 *     ;;   call_indirect (type $cont) [data, code=0] @ table_index=cb
 *     (func (export "uniffi_poll") (param i64 i32 i64) ...))
 *
 * The continuation shape does *not* follow the (handle, ...args) UniFFI
 * vtable convention. The host pushes (data: u64, code: i8/i32) directly and
 * the continuation route in CallbackTable.dispatch bypasses the
 * registry-by-handle lookup, treating `data` as a u64 cookie that maps to a
 * resolver in `FutureRegistry`.
 */
// host wasm bytes (length=352)
// trampoline blob length: 81
const FUTURE_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 32, 5, 96, 2, 127, 127, 1, 127, 96, 3, 127,
  127, 127, 0, 96, 4, 127, 127, 127, 127, 1, 127, 96, 3, 126, 127, 126, 0, 96,
  2, 126, 127, 0, 2, 1, 0, 3, 5, 4, 0, 1, 2, 3, 4, 4, 1, 112, 0, 4, 5, 3, 1, 0,
  4, 6, 7, 1, 127, 1, 65, 128, 8, 11, 7, 106, 6, 6, 109, 101, 109, 111, 114,
  121, 2, 0, 25, 95, 95, 105, 110, 100, 105, 114, 101, 99, 116, 95, 102, 117,
  110, 99, 116, 105, 111, 110, 95, 116, 97, 98, 108, 101, 1, 0, 12, 95, 95, 117,
  98, 114, 110, 95, 97, 108, 108, 111, 99, 0, 0, 11, 95, 95, 117, 98, 114, 110,
  95, 102, 114, 101, 101, 0, 1, 22, 95, 95, 117, 98, 114, 110, 95, 101, 109,
  105, 116, 95, 116, 114, 97, 109, 112, 111, 108, 105, 110, 101, 0, 2, 11, 117,
  110, 105, 102, 102, 105, 95, 112, 111, 108, 108, 0, 3, 9, 1, 0, 10, 78, 4, 32,
  1, 1, 127, 35, 0, 32, 1, 65, 1, 107, 106, 32, 1, 65, 1, 107, 65, 127, 115,
  113, 33, 2, 32, 2, 32, 0, 106, 36, 0, 32, 2, 11, 2, 0, 11, 28, 0, 32, 1, 65,
  209, 0, 73, 4, 64, 65, 0, 15, 11, 32, 0, 65, 0, 65, 209, 0, 252, 10, 0, 0, 65,
  209, 0, 11, 11, 0, 32, 2, 65, 0, 32, 1, 17, 4, 0, 11, 11, 87, 1, 0, 65, 0, 11,
  81, 0, 97, 115, 109, 1, 0, 0, 0, 1, 12, 2, 96, 2, 126, 127, 0, 96, 3, 127,
  126, 127, 0, 2, 23, 1, 3, 101, 110, 118, 15, 95, 95, 117, 98, 114, 110, 95,
  100, 105, 115, 112, 97, 116, 99, 104, 0, 1, 3, 2, 1, 0, 7, 14, 1, 10, 116,
  114, 97, 109, 112, 111, 108, 105, 110, 101, 0, 1, 10, 12, 1, 10, 0, 65, 0, 32,
  0, 32, 1, 16, 0, 11,
]);

test("RustFutureContinuation resolves the awaiting promise", async () => {
  const mod = await UniffiNativeModule.open(FUTURE_BYTES);
  await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: {},
    structs: {},
  });

  // Install the continuation trampoline at table slot 0; this is the index
  // the host wasm expects.
  const idx = mod.futures.installContinuation(0);
  assert.strictEqual(idx, 0);

  const code = await new Promise<number>((resolve) => {
    const data = mod.futures.allocateContinuation(resolve);
    // uniffi_poll(handle, cb, data) — synchronously resolves to READY=0.
    (mod.exports.uniffi_poll as any)(0n, idx, data);
  });
  assert.strictEqual(code, 0); // READY = 0
});

test("Multiple in-flight continuations dispatch by data cookie", async () => {
  const mod = await UniffiNativeModule.open(FUTURE_BYTES);
  await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: {},
    structs: {},
  });

  mod.futures.installContinuation(0);

  const got: { data: bigint; code: number }[] = [];
  const p1 = new Promise<number>((resolve) => {
    const d = mod.futures.allocateContinuation((c) => {
      got.push({ data: d, code: c });
      resolve(c);
    });
    (mod.exports.uniffi_poll as any)(0n, 0, d);
  });
  const p2 = new Promise<number>((resolve) => {
    const d = mod.futures.allocateContinuation((c) => {
      got.push({ data: d, code: c });
      resolve(c);
    });
    (mod.exports.uniffi_poll as any)(0n, 0, d);
  });

  await Promise.all([p1, p2]);
  assert.strictEqual(got.length, 2);
  assert.notStrictEqual(got[0].data, got[1].data);
});
