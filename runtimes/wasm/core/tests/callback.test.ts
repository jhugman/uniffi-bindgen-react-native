/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { UniffiNativeModule } from "../src/module.js";
import { FfiType } from "../src/ffi-type.js";

/**
 * Pre-assembled bytes for a synthetic host wasm module that lets us exercise
 * the JS-side CallbackTable end-to-end without the helper crate's cdylib.
 *
 * Shape:
 *
 *   (module
 *     (memory (export "memory") 1)
 *     (table (export "__indirect_function_table") 4 funcref)
 *     (type $cb (func (param i32 i32) (result i32))) ;; (handle, x) -> i32
 *
 *     ;; Bump-allocator backed by a mutable global at 1024.
 *     (func (export "__ubrn_alloc") (param i32 i32) (result i32) ...)
 *     (func (export "__ubrn_free") (param i32 i32 i32))
 *
 *     ;; __ubrn_emit_trampoline(out_ptr, out_cap, _sig_ptr, _sig_len) -> i32
 *     ;; ignores the descriptor and copies a pre-baked trampoline blob
 *     ;; (shapeId=0, params=[i32,i32], ret=i32) from memory[0..len) to out_ptr.
 *     (func (export "__ubrn_emit_trampoline") ...)
 *
 *     ;; uniffi_invoke(x): call_indirect (type $cb) (handle=1) (x) (table_index=0)
 *     (func (export "uniffi_invoke") (param i32) (result i32) ...))
 *
 * Per the UniFFI vtable convention `fn(self_handle, args...)`, the host
 * passes the *handle* as the first argument; the JS dispatcher in
 * `callback.ts` strips it to look up the closure.
 */
// host wasm bytes (length=359)
// trampoline blob length: 83
const CB_HOST_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 35, 6, 96, 0, 0, 96, 2, 127, 127, 1, 127, 96,
  3, 127, 127, 127, 0, 96, 4, 127, 127, 127, 127, 1, 127, 96, 1, 127, 1, 127,
  96, 2, 127, 127, 1, 127, 2, 1, 0, 3, 5, 4, 1, 2, 3, 4, 4, 4, 1, 112, 0, 4, 5,
  3, 1, 0, 4, 6, 7, 1, 127, 1, 65, 128, 8, 11, 7, 108, 6, 6, 109, 101, 109, 111,
  114, 121, 2, 0, 25, 95, 95, 105, 110, 100, 105, 114, 101, 99, 116, 95, 102,
  117, 110, 99, 116, 105, 111, 110, 95, 116, 97, 98, 108, 101, 1, 0, 12, 95, 95,
  117, 98, 114, 110, 95, 97, 108, 108, 111, 99, 0, 0, 11, 95, 95, 117, 98, 114,
  110, 95, 102, 114, 101, 101, 0, 1, 22, 95, 95, 117, 98, 114, 110, 95, 101,
  109, 105, 116, 95, 116, 114, 97, 109, 112, 111, 108, 105, 110, 101, 0, 2, 13,
  117, 110, 105, 102, 102, 105, 95, 105, 110, 118, 111, 107, 101, 0, 3, 9, 1, 0,
  10, 78, 4, 32, 1, 1, 127, 35, 0, 32, 1, 65, 1, 107, 106, 32, 1, 65, 1, 107,
  65, 127, 115, 113, 33, 2, 32, 2, 32, 0, 106, 36, 0, 32, 2, 11, 2, 0, 11, 28,
  0, 32, 1, 65, 211, 0, 73, 4, 64, 65, 0, 15, 11, 32, 0, 65, 0, 65, 211, 0, 252,
  10, 0, 0, 65, 211, 0, 11, 11, 0, 65, 1, 32, 0, 65, 0, 17, 5, 0, 11, 11, 89, 1,
  0, 65, 0, 11, 83, 0, 97, 115, 109, 1, 0, 0, 0, 1, 14, 2, 96, 2, 127, 127, 1,
  127, 96, 3, 127, 127, 127, 1, 127, 2, 23, 1, 3, 101, 110, 118, 15, 95, 95,
  117, 98, 114, 110, 95, 100, 105, 115, 112, 97, 116, 99, 104, 0, 1, 3, 2, 1, 0,
  7, 14, 1, 10, 116, 114, 97, 109, 112, 111, 108, 105, 110, 101, 0, 1, 10, 12,
  1, 10, 0, 65, 0, 32, 0, 32, 1, 16, 0, 11,
]);

test("callback round-trip through trampoline + dispatch", async () => {
  const mod = await UniffiNativeModule.open(CB_HOST_BYTES);

  // The "Doubler" callback shape: (handle: i32, x: i32) -> i32.
  // The first arg is the registry handle (UniFFI vtable convention); the
  // user-visible argument is `x`.
  const doublerDef = {
    args: [FfiType.Int32, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: false,
  };

  const nm = await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {
      uniffi_invoke: {
        args: [FfiType.Int32],
        ret: FfiType.Int32,
        hasRustCallStatus: false,
      },
    },
    callbacks: { Doubler: doublerDef },
    structs: {},
  });

  // Register a JS callback. The trampoline pushes (shape_id, handle, x); the
  // JS dispatcher strips both shape_id AND handle before invoking the
  // closure, so the closure receives only the user args.
  const handle = mod.callbacks.register(
    "Doubler",
    (x: number) => x * 2,
    doublerDef,
  );
  assert.strictEqual(
    handle,
    1,
    "first registration on this shape should be handle=1",
  );

  const shapeId = mod.callbacks.defineShape("Doubler", doublerDef);
  mod.callbacks.installAt(0, shapeId);

  // Host's `uniffi_invoke(21)` does `call_indirect[0](handle=1, 21)`, which
  // hits the trampoline, which pushes shape_id=0 and forwards (1, 21) to
  // dispatch. We recover the closure from (shapeId=0, handle=1) and invoke
  // it with (21,), returning 42.
  assert.strictEqual(nm.uniffi_invoke(21), 42);
});

test("multiple registrations of the same shape get distinct handles", async () => {
  const mod = await UniffiNativeModule.open(CB_HOST_BYTES);
  const def = {
    args: [FfiType.Int32, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: false,
  };
  await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: { Shape: def },
    structs: {},
  });

  const h1 = mod.callbacks.register("Shape", (x: number) => x, def);
  const h2 = mod.callbacks.register("Shape", (x: number) => x, def);
  assert.notStrictEqual(h1, h2);
});

test("unregister removes the closure but keeps the trampoline", async () => {
  const mod = await UniffiNativeModule.open(CB_HOST_BYTES);
  const def = {
    args: [FfiType.Int32, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: false,
  };
  const nm = await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {
      uniffi_invoke: {
        args: [FfiType.Int32],
        ret: FfiType.Int32,
        hasRustCallStatus: false,
      },
    },
    callbacks: { Shape: def },
    structs: {},
  });

  const h = mod.callbacks.register("Shape", (x: number) => x + 1, def);
  const shapeId = mod.callbacks.defineShape("Shape", def);
  mod.callbacks.installAt(0, shapeId);
  assert.strictEqual(nm.uniffi_invoke(5), 6);

  mod.callbacks.unregister(h, def);
  assert.throws(() => nm.uniffi_invoke(5), /unknown handle/);
});

test("callback registry shrinks on unregister", async () => {
  const mod = await UniffiNativeModule.open(CB_HOST_BYTES);
  const def = {
    args: [FfiType.Int32, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: false,
  };
  await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: { Doubler: def },
    structs: {},
  });
  const handle = mod.callbacks.register("Doubler", () => 0, def);
  assert.strictEqual(mod.callbacks.size(def), 1);
  mod.callbacks.unregister(handle, def);
  assert.strictEqual(mod.callbacks.size(def), 0);
});

// Shape ids have no ceiling: `trampoline.ts` emits one as a multi-byte signed
// LEB128 `i32.const` and `__ubrn_dispatch` takes it as an i32. These tests pin
// that, and the property that keeps ids scarce anyway — instances share a
// shape, so shape use is bounded by the API surface.

const SHAPE_DEF = {
  args: [FfiType.Int32, FfiType.Int32],
  ret: FfiType.Int32,
  hasRustCallStatus: false,
};

async function openHost() {
  const mod = await UniffiNativeModule.open(CB_HOST_BYTES);
  await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {},
    callbacks: { Shape: SHAPE_DEF },
    structs: {},
  });
  return mod;
}

test("registering many instances does not consume shapes", async () => {
  const mod = await openHost();
  const before = mod.callbacks.defineShape("Shape", SHAPE_DEF);
  for (let i = 0; i < 1000; i++) {
    mod.callbacks.register("Shape", (x: number) => x + i, SHAPE_DEF);
  }
  const after = mod.callbacks.defineShape("Shape", SHAPE_DEF);
  assert.strictEqual(
    after,
    before,
    "1000 registrations should reuse one shape, not allocate more",
  );
  assert.strictEqual(mod.callbacks.size(SHAPE_DEF), 1000);
});

test("installing the same closure repeatedly reuses its slot", async () => {
  const mod = await openHost();
  const fn = (x: number) => x;
  const first = mod.callbacks.installCallbackFunction(fn, SHAPE_DEF);
  for (let i = 0; i < 100; i++) {
    assert.strictEqual(
      mod.callbacks.installCallbackFunction(fn, SHAPE_DEF),
      first,
      "same (closure, signature) must reuse the installed slot",
    );
  }
});

test("shape ids are not capped at 255", async () => {
  const mod = await openHost();
  // Distinct closures each take their own shape by design, so 300 of them
  // means 300 shapes.
  let slot = -1;
  for (let i = 0; i < 300; i++) {
    slot = mod.callbacks.installCallbackFunction(
      (x: number) => x + i,
      SHAPE_DEF,
    );
  }
  assert.ok(slot > 255, `expected a slot past the old cap, got ${slot}`);
});
