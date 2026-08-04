/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { UniffiNativeModule } from "../src/module.js";

/**
 * Pre-assembled bytes for a minimal host wasm module that exports the
 * helper-crate symbols our player expects. Generated via:
 *
 *   wat2wasm /tmp/host.wat -o /tmp/host.wasm
 *
 * from the following .wat (the import comes first so import indices line up):
 *
 *   (module
 *     (import "env" "__ubrn_panic_log" (func (param i32 i32)))
 *     (memory (export "memory") 1)
 *     (func (export "__ubrn_alloc") (param i32 i32) (result i32) i32.const 1024)
 *     (func (export "__ubrn_free") (param i32 i32 i32))
 *     (func (export "__ubrn_install_panic_hook"))
 *     (func (export "__ubrn_emit_trampoline") (param i32 i32 i32 i32) (result i32) i32.const 0)
 *   )
 */
const HOST_WASM_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 29, 5, 96, 2, 127, 127, 0, 96, 2, 127, 127, 1,
  127, 96, 3, 127, 127, 127, 0, 96, 0, 0, 96, 4, 127, 127, 127, 127, 1, 127, 2,
  24, 1, 3, 101, 110, 118, 16, 95, 95, 117, 98, 114, 110, 95, 112, 97, 110, 105,
  99, 95, 108, 111, 103, 0, 0, 3, 5, 4, 1, 2, 3, 4, 5, 3, 1, 0, 1, 7, 92, 5, 6,
  109, 101, 109, 111, 114, 121, 2, 0, 12, 95, 95, 117, 98, 114, 110, 95, 97,
  108, 108, 111, 99, 0, 1, 11, 95, 95, 117, 98, 114, 110, 95, 102, 114, 101,
  101, 0, 2, 25, 95, 95, 117, 98, 114, 110, 95, 105, 110, 115, 116, 97, 108,
  108, 95, 112, 97, 110, 105, 99, 95, 104, 111, 111, 107, 0, 3, 22, 95, 95, 117,
  98, 114, 110, 95, 101, 109, 105, 116, 95, 116, 114, 97, 109, 112, 111, 108,
  105, 110, 101, 0, 4, 10, 18, 4, 5, 0, 65, 128, 8, 11, 2, 0, 11, 2, 0, 11, 4,
  0, 65, 0, 11,
]);

test("UniffiNativeModule.open resolves required helper exports", async () => {
  const mod = await UniffiNativeModule.open(HOST_WASM_BYTES);
  assert.ok(mod);
  assert.ok(mod.memory);
  assert.ok(mod.scratch);
  assert.ok("__ubrn_alloc" in mod.exports);
  assert.ok("__ubrn_emit_trampoline" in mod.exports);
});

test("UniffiNativeModule.open rejects when __ubrn_alloc is missing", async () => {
  // Smallest valid wasm: just the magic bytes + version. WebAssembly.instantiate
  // accepts this (it produces a module with no imports and no exports), so the
  // rejection comes from our own missing-export check inside the constructor.
  const bytes = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);
  await assert.rejects(() => UniffiNativeModule.open(bytes), /__ubrn_alloc/);
});

test("registered module exposes rustbuffer_alloc / rustbuffer_free", async () => {
  const mod = await UniffiNativeModule.open(HOST_WASM_BYTES);
  const nm = await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: {},
    structs: {},
  });

  assert.strictEqual(typeof nm.rustbuffer_alloc, "function");
  assert.strictEqual(typeof nm.rustbuffer_free, "function");

  const view = nm.rustbuffer_alloc(16) as Uint8Array;
  assert.ok(
    view instanceof Uint8Array,
    "rustbuffer_alloc returns a Uint8Array",
  );
  assert.strictEqual(view.byteLength, 16);
  assert.strictEqual(
    view.buffer,
    mod.memory.buffer(),
    "view aliases wasm linear memory directly",
  );

  // Mutate the view — round-trip through Memory.readBytes to confirm the
  // bytes landed inside wasm memory at view.byteOffset.
  view[0] = 0xab;
  view[15] = 0xcd;
  const readBack = mod.memory.readBytes(view.byteOffset, 16);
  assert.strictEqual(readBack[0], 0xab);
  assert.strictEqual(readBack[15], 0xcd);

  // Symmetric free — accepts the Uint8Array and reconstructs (ptr, len).
  // The host wasm's __ubrn_free is a no-op stub, so this just verifies
  // the call shape works.
  nm.rustbuffer_free(view);
});

test("rustbuffer_alloc(0) returns an empty view without allocating", async () => {
  const mod = await UniffiNativeModule.open(HOST_WASM_BYTES);
  const nm = await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: {},
    structs: {},
  });

  const empty = nm.rustbuffer_alloc(0) as Uint8Array;
  assert.ok(empty instanceof Uint8Array);
  assert.strictEqual(empty.byteLength, 0);
  // Free of a zero-length view must be a safe no-op (don't call __ubrn_free
  // with a 0-length range, which would be a no-op anyway but the ptr may be 0).
  nm.rustbuffer_free(empty);
});
