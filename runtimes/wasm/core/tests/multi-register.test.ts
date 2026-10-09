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
 * One wasm module hosting several uniffi crates: every crate registers its
 * own definitions table, and a crate's callbacks must lift their arguments
 * against that crate's table however many crates register after it.
 *
 * Assembled with `wat2wasm` from:
 *
 *   (module
 *     (type $cb (func (param i32 i32)))
 *     (memory (export "memory") 1)
 *     (table (export "__indirect_function_table") 4 funcref)
 *     (global $next (mut i32) (i32.const 1024))
 *     (global $slot (mut i32) (i32.const 0))
 *     (func (export "__ubrn_alloc") (param i32 i32) (result i32) ...bump...)
 *     (func (export "__ubrn_free") (param i32 i32 i32))
 *     ;; uniffi_set_listener(slot): remember a table index
 *     (func (export "uniffi_set_listener") (param i32) (global.set $slot (local.get 0)))
 *     ;; uniffi_invoke(x): call_indirect (type $cb) (handle=1) (x) (slot)
 *     (func (export "uniffi_invoke") (param i32)
 *       (call_indirect (type $cb) (i32.const 1) (local.get 0) (global.get $slot))))
 */
const HOST_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 22, 4, 96, 2, 127, 127, 0, 96, 2, 127, 127, 1,
  127, 96, 3, 127, 127, 127, 0, 96, 1, 127, 0, 3, 5, 4, 1, 2, 3, 3, 4, 4, 1,
  112, 0, 4, 5, 3, 1, 0, 1, 6, 12, 2, 127, 1, 65, 128, 8, 11, 127, 1, 65, 0, 11,
  7, 105, 6, 6, 109, 101, 109, 111, 114, 121, 2, 0, 25, 95, 95, 105, 110, 100,
  105, 114, 101, 99, 116, 95, 102, 117, 110, 99, 116, 105, 111, 110, 95, 116,
  97, 98, 108, 101, 1, 0, 12, 95, 95, 117, 98, 114, 110, 95, 97, 108, 108, 111,
  99, 0, 0, 11, 95, 95, 117, 98, 114, 110, 95, 102, 114, 101, 101, 0, 1, 19,
  117, 110, 105, 102, 102, 105, 95, 115, 101, 116, 95, 108, 105, 115, 116, 101,
  110, 101, 114, 0, 2, 13, 117, 110, 105, 102, 102, 105, 95, 105, 110, 118, 111,
  107, 101, 0, 3, 10, 41, 4, 17, 1, 1, 127, 35, 0, 33, 2, 35, 0, 32, 0, 106, 36,
  0, 32, 2, 11, 2, 0, 11, 6, 0, 32, 0, 36, 1, 11, 11, 0, 65, 1, 32, 0, 35, 1,
  17, 0, 0, 11,
]);

const symbols = {
  rustbuffer_alloc: "_",
  rustbuffer_free: "_",
  rustbuffer_from_bytes: "_",
};

test("a crate's callback lifts a Callback arg by its own table after later crates register", async () => {
  const mod = await UniffiNativeModule.open(HOST_BYTES);

  // Crate A: an async-style listener whose second argument is a completion
  // callback, as uniffi's ForeignFutureComplete* shapes are.
  const a = mod.registerSync({
    symbols,
    functions: {
      uniffi_set_listener: {
        args: [FfiType.Callback("Listener")],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      uniffi_invoke: {
        args: [FfiType.Int32],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    callbacks: {
      Listener: {
        args: [FfiType.Int32, FfiType.Callback("Complete")],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      Complete: {
        args: [FfiType.Int32],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    structs: {},
  });
  // Crate B registers last and knows nothing of A's callbacks.
  mod.registerSync({ symbols, functions: {}, callbacks: {}, structs: {} });

  // Routed callbacks receive every wasm arg verbatim, instance handle first.
  const completed: number[] = [];
  a.uniffi_set_listener((_handle: number, complete: (x: number) => void) => {
    complete(5);
  });
  const completeSlot = mod.callbacks.installCallbackFunction(
    (x: number) => {
      completed.push(x);
    },
    { args: [FfiType.Int32], ret: FfiType.Void, hasRustCallStatus: false },
  );
  a.uniffi_invoke(completeSlot);
  assert.deepStrictEqual(completed, [5]);
});
