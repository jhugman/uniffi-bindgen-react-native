/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { beforeEach, test } from "node:test";
import assert from "node:assert";
import {
  uniffiRustFutureHandleCount,
  type ModuleDefinitions,
} from "@ubjs/core";
import { FfiType } from "../src/ffi-type.js";
import {
  UniffiNativeModule,
  type NativeModuleInterface,
} from "../src/module.js";

/**
 * A synthetic future host. `start(n)` reports MAYBE_READY `n` times before
 * READY; `poll` calls the continuation at once, as a future that is already
 * woken does. Assembled with `wat2wasm host.wat -o host.wasm` from:
 *
 *   (module
 *     (type $cont (func (param i64 i32)))
 *     (memory (export "memory") 3)
 *     (table (export "__indirect_function_table") 1 funcref)
 *     (data (i32.const 2048) "\01\02\03")
 *     (global $next (mut i32) (i32.const 4096))
 *     (global $polls (mut i32) (i32.const 0))
 *     (global $cancelled (mut i32) (i32.const 0))
 *     (global $freed (mut i32) (i32.const 0))
 *     (global $frees (mut i32) (i32.const 0))
 *     (func (export "__ubrn_alloc") (param $size i32) (param $align i32) (result i32)
 *       (local $p i32)
 *       (local.set $p (global.get $next))
 *       (global.set $next (i32.add (global.get $next)
 *         (i32.and (i32.add (local.get $size) (i32.const 7)) (i32.const -8))))
 *       (local.get $p))
 *     (func (export "__ubrn_free") (param i32 i32 i32)
 *       (global.set $frees (i32.add (global.get $frees) (i32.const 1))))
 *     ;; start(n): the future reports MAYBE_READY n times before READY
 *     (func (export "start") (param $n i32) (result i64)
 *       (global.set $polls (local.get $n))
 *       (i64.const 7))
 *     (func (export "start_rb") (result i64)
 *       (global.set $polls (i32.const 0))
 *       (i64.const 8))
 *     ;; poll(handle, cb, data): calls table[cb](data, code) at once
 *     (func (export "poll") (param $h i64) (param $cb i32) (param $data i64)
 *       (local $code i32)
 *       (if (i32.gt_s (global.get $polls) (i32.const 0))
 *         (then
 *           (global.set $polls (i32.sub (global.get $polls) (i32.const 1)))
 *           (local.set $code (i32.const 1)))
 *         (else (local.set $code (i32.const 0))))
 *       (call_indirect (type $cont) (local.get $data) (local.get $code) (local.get $cb)))
 *     (func (export "cancel") (param i64)
 *       (global.set $cancelled (i32.const 1))
 *       (global.set $polls (i32.const 0)))
 *     ;; complete(handle, status) -> u32: code 3 once cancelled, value 42
 *     (func (export "complete") (param $h i64) (param $status i32) (result i32)
 *       (i32.store8 (local.get $status)
 *         (select (i32.const 3) (i32.const 0) (global.get $cancelled)))
 *       (i32.const 42))
 *     ;; complete_rb(sret, handle, status): RustBuffer { capacity 8, len 3, data 2048 }
 *     (func (export "complete_rb") (param $sret i32) (param $h i64) (param $status i32)
 *       (i64.store (local.get $sret) (i64.const 8))
 *       (i64.store offset=8 (local.get $sret) (i64.const 3))
 *       (i32.store offset=16 (local.get $sret) (i32.const 2048)))
 *     (func (export "free") (param i64)
 *       (global.set $freed (i32.add (global.get $freed) (i32.const 1))))
 *     (func (export "freed") (result i32) (global.get $freed))
 *     (func (export "frees") (result i32) (global.get $frees))
 *     (func (export "reset")
 *       (global.set $cancelled (i32.const 0))
 *       (global.set $freed (i32.const 0))
 *       (global.set $frees (i32.const 0))))
 */
const ASYNC_HOST_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 56, 11, 96, 2, 126, 127, 0, 96, 2, 127, 127,
  1, 127, 96, 3, 127, 127, 127, 0, 96, 1, 127, 1, 126, 96, 0, 1, 126, 96, 3,
  126, 127, 126, 0, 96, 1, 126, 0, 96, 2, 126, 127, 1, 127, 96, 3, 127, 126,
  127, 0, 96, 0, 1, 127, 96, 0, 0, 3, 13, 12, 1, 2, 3, 4, 5, 6, 7, 8, 6, 9, 9,
  10, 4, 4, 1, 112, 0, 1, 5, 3, 1, 0, 3, 6, 27, 5, 127, 1, 65, 128, 32, 11, 127,
  1, 65, 0, 11, 127, 1, 65, 0, 11, 127, 1, 65, 0, 11, 127, 1, 65, 0, 11, 7, 158,
  1, 14, 6, 109, 101, 109, 111, 114, 121, 2, 0, 25, 95, 95, 105, 110, 100, 105,
  114, 101, 99, 116, 95, 102, 117, 110, 99, 116, 105, 111, 110, 95, 116, 97, 98,
  108, 101, 1, 0, 12, 95, 95, 117, 98, 114, 110, 95, 97, 108, 108, 111, 99, 0,
  0, 11, 95, 95, 117, 98, 114, 110, 95, 102, 114, 101, 101, 0, 1, 5, 115, 116,
  97, 114, 116, 0, 2, 8, 115, 116, 97, 114, 116, 95, 114, 98, 0, 3, 4, 112, 111,
  108, 108, 0, 4, 6, 99, 97, 110, 99, 101, 108, 0, 5, 8, 99, 111, 109, 112, 108,
  101, 116, 101, 0, 6, 11, 99, 111, 109, 112, 108, 101, 116, 101, 95, 114, 98,
  0, 7, 4, 102, 114, 101, 101, 0, 8, 5, 102, 114, 101, 101, 100, 0, 9, 5, 102,
  114, 101, 101, 115, 0, 10, 5, 114, 101, 115, 101, 116, 0, 11, 10, 179, 1, 12,
  23, 1, 1, 127, 35, 0, 33, 2, 35, 0, 32, 0, 65, 7, 106, 65, 120, 113, 106, 36,
  0, 32, 2, 11, 9, 0, 35, 4, 65, 1, 106, 36, 4, 11, 8, 0, 32, 0, 36, 1, 66, 7,
  11, 8, 0, 65, 0, 36, 1, 66, 8, 11, 37, 1, 1, 127, 35, 1, 65, 0, 74, 4, 64, 35,
  1, 65, 1, 107, 36, 1, 65, 1, 33, 3, 5, 65, 0, 33, 3, 11, 32, 2, 32, 3, 32, 1,
  17, 0, 0, 11, 10, 0, 65, 1, 36, 2, 65, 0, 36, 1, 11, 16, 0, 32, 1, 65, 3, 65,
  0, 35, 2, 27, 58, 0, 0, 65, 42, 11, 24, 0, 32, 0, 66, 8, 55, 3, 0, 32, 0, 66,
  3, 55, 3, 8, 32, 0, 65, 128, 16, 54, 2, 16, 11, 9, 0, 35, 3, 65, 1, 106, 36,
  3, 11, 4, 0, 35, 3, 11, 4, 0, 35, 4, 11, 14, 0, 65, 0, 36, 2, 65, 0, 36, 3,
  65, 0, 36, 4, 11, 11, 10, 1, 0, 65, 128, 16, 11, 3, 1, 2, 3,
]);

const defs = {
  symbols: {
    rustbuffer_alloc: "_",
    rustbuffer_free: "_",
    rustbuffer_from_bytes: "_",
  },
  functions: {
    start: {
      args: [FfiType.UInt32],
      ret: FfiType.Handle,
      hasRustCallStatus: false,
      async: {
        poll: "poll",
        cancel: "cancel",
        complete: "complete",
        free: "free",
        ret: FfiType.UInt32,
      },
    },
    start_rb: {
      args: [],
      ret: FfiType.Handle,
      hasRustCallStatus: false,
      async: {
        poll: "poll",
        cancel: "cancel",
        complete: "complete_rb",
        free: "free",
        ret: FfiType.RustBuffer,
      },
    },
  },
  callbacks: {},
  structs: {},
} satisfies ModuleDefinitions;

let mod: UniffiNativeModule;
let nm: NativeModuleInterface;

beforeEach(async () => {
  mod = await UniffiNativeModule.open(ASYNC_HOST_BYTES);
  (mod.exports.reset as () => void)();
  nm = mod.registerSync(defs);
});

test("an async entry polls to READY on microtasks and resolves with complete's value; nothing is left behind", async () => {
  // the table carries no continuation callback: the player supplies the shape
  let macrotaskRan = false;
  setTimeout(() => {
    macrotaskRan = true;
  }, 0);
  const status = { code: 0 };
  assert.strictEqual(await nm.start(2, status, undefined), 42);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(macrotaskRan, false);
  assert.strictEqual((mod.exports.freed as () => number)(), 1);
  assert.strictEqual(uniffiRustFutureHandleCount(), 0);
});

test("a continuation shape in the table does not displace the player's own", async () => {
  const own = mod.registerSync({
    ...defs,
    callbacks: {
      // Not UniFFI's shape: a poll lowering through it traps in call_indirect.
      RustFutureContinuationCallback: {
        args: [FfiType.UInt32, FfiType.UInt32],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
  });
  assert.strictEqual(await own.start(2, { code: 0 }, undefined), 42);
});

test("an abort through the signal cancels the future and complete reports CALL_CANCELLED", async () => {
  const controller = new AbortController();
  const status = { code: 0 };
  const p = nm.start(1000, status, controller.signal);
  controller.abort();
  // A failed call carries no value, as with a synchronous call's error path.
  assert.strictEqual(await p, undefined);
  assert.strictEqual(status.code, 3);
  assert.strictEqual((mod.exports.freed as () => number)(), 1);
  assert.strictEqual(uniffiRustFutureHandleCount(), 0);
});

test("async functions sharing future exports specialize each of them once", async () => {
  // Every specialization reserves scratch for good, so count reservations.
  let reserves = 0;
  const reserve = mod.scratch.reserve.bind(mod.scratch);
  mod.scratch.reserve = (size: number) => {
    reserves += 1;
    return reserve(size);
  };
  mod.registerSync({ ...defs, functions: { start: defs.functions.start } });
  const forOne = reserves;

  reserves = 0;
  const shared = mod.registerSync({
    ...defs,
    functions: {
      start: defs.functions.start,
      // The same poll, cancel, complete and free as `start`.
      start_rb: {
        ...defs.functions.start_rb,
        async: defs.functions.start.async,
      },
    },
  });
  // The second function adds only its own start.
  assert.strictEqual(reserves, forOne + 1);
  assert.strictEqual(await shared.start(1, { code: 0 }, undefined), 42);
  assert.strictEqual(await shared.start_rb({ code: 0 }, undefined), 42);
});

test("a RustBuffer result is copied onto the JS heap and its wasm allocation freed before the entry resolves", async () => {
  const v = (await nm.start_rb({ code: 0 }, undefined)) as Uint8Array;
  assert.deepStrictEqual(Array.from(v), [1, 2, 3]);
  assert.notStrictEqual(v.buffer, mod.memory.buffer());
  assert.strictEqual((mod.exports.frees as () => number)(), 1);
  nm.rustbuffer_free(v); // the call site's free: a no-op on a copy
  assert.strictEqual((mod.exports.frees as () => number)(), 1);
});

test("a future export the table names but the module lacks fails registration by name", () => {
  assert.throws(
    () =>
      mod.registerSync({
        ...defs,
        functions: {
          start: {
            ...defs.functions.start,
            async: { ...defs.functions.start.async, poll: "nope" },
          },
        },
      }),
    /wasm export "nope" not found/,
  );
});

test("rustbuffer_free frees a view over wasm memory and refuses one that memory growth detached", () => {
  // complete_rb registered as a plain call hands back a view over wasm memory.
  const sync = mod.registerSync({
    ...defs,
    functions: {
      complete_rb: {
        args: [FfiType.Handle],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });
  const live = sync.complete_rb(8n, { code: 0 }) as Uint8Array;
  sync.rustbuffer_free(live);
  assert.strictEqual((mod.exports.frees as () => number)(), 1);

  const stale = sync.complete_rb(8n, { code: 0 }) as Uint8Array;
  (mod.exports.memory as WebAssembly.Memory).grow(1);
  assert.throws(() => sync.rustbuffer_free(stale), /detached/);
  assert.strictEqual((mod.exports.frees as () => number)(), 1);
});
