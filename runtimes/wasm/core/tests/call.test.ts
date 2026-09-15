/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { UniffiNativeModule } from "../src/module.js";
import { FfiType } from "../src/ffi-type.js";
import {
  buildJitDispatcher,
  canUseFunctionConstructor,
  specializeFunction,
  type DispatchContext,
  type FunctionDef,
} from "../src/call.js";
import { Memory } from "../src/memory.js";
import { Scratch } from "../src/scratch.js";

/**
 * Pre-assembled bytes for a synthetic wasm module with one user function and
 * the required helper exports. Generated via:
 *
 *   wat2wasm /tmp/host-call.wat -o /tmp/host-call.wasm
 *
 * from the following .wat:
 *
 *   (module
 *     (memory (export "memory") 1)
 *     (func (export "uniffi_test_add") (param i32 i32 i32) (result i32)
 *       local.get 0 local.get 1 i32.add)
 *     (func (export "__ubrn_alloc") (param i32 i32) (result i32) i32.const 1024)
 *     (func (export "__ubrn_free") (param i32 i32 i32))
 *     (func (export "__ubrn_emit_trampoline") (param i32 i32 i32 i32) (result i32) i32.const 0)
 *   )
 *
 * `uniffi_test_add` declares three i32 params because the UniFFI Rust call
 * convention passes the `out_status` pointer as the LAST i32. The wasm body
 * only adds the first two; the third (status pointer) is unused but must be
 * present in the type.
 */
const HOST_BYTES = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 28, 4, 96, 3, 127, 127, 127, 1, 127, 96, 2,
  127, 127, 1, 127, 96, 3, 127, 127, 127, 0, 96, 4, 127, 127, 127, 127, 1, 127,
  3, 5, 4, 0, 1, 2, 3, 5, 3, 1, 0, 1, 7, 82, 5, 6, 109, 101, 109, 111, 114, 121,
  2, 0, 15, 117, 110, 105, 102, 102, 105, 95, 116, 101, 115, 116, 95, 97, 100,
  100, 0, 0, 12, 95, 95, 117, 98, 114, 110, 95, 97, 108, 108, 111, 99, 0, 1, 11,
  95, 95, 117, 98, 114, 110, 95, 102, 114, 101, 101, 0, 2, 22, 95, 95, 117, 98,
  114, 110, 95, 101, 109, 105, 116, 95, 116, 114, 97, 109, 112, 111, 108, 105,
  110, 101, 0, 3, 10, 23, 4, 7, 0, 32, 0, 32, 1, 106, 11, 5, 0, 65, 128, 8, 11,
  2, 0, 11, 4, 0, 65, 0, 11,
]);

test("specialized dispatcher calls the wasm fn and reads sret/status", async () => {
  const mod = await UniffiNativeModule.open(HOST_BYTES);
  const nm = await mod.register({
    symbols: {
      rustbuffer_alloc: "_",
      rustbuffer_free: "_",
      rustbuffer_from_bytes: "_",
    },
    functions: {
      uniffi_test_add: {
        args: [FfiType.Int32, FfiType.Int32],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
    callbacks: {},
    structs: {},
  });
  const status = { code: 0 };
  const result = nm.uniffi_test_add(3, 4, status);
  assert.strictEqual(result, 7);
  assert.strictEqual(status.code, 0);
});

// No RustBuffer round-trip here: a synthetic one needs a working bump
// allocator inside the test wasm. The fixture suite covers it end to end —
// every fixture using a record or string crosses planArg + planRet with a
// real wasm-side allocator.

test("canUseFunctionConstructor returns true in Node without CSP", () => {
  assert.strictEqual(canUseFunctionConstructor(), true);
});

/**
 * Hand `buildJitDispatcher` a stub exportFn and a synthetic memory/scratch
 * backed by a plain WebAssembly.Memory, and check that the JIT body
 *   - zeros the status region before the call,
 *   - forwards the user arg through to exportFn unchanged (modulo |0),
 *   - appends the status pointer to the wasm-side arg list,
 *   - reads the u8 status byte after the call,
 *   - returns the scalar value the exportFn produced.
 */
test("buildJitDispatcher emits a body that calls fn + reads status (u32 → u32)", () => {
  const wasmMem = new WebAssembly.Memory({ initial: 1 });
  const memory = new Memory(wasmMem);
  // Pretend bump-allocator: hand out monotonically increasing offsets,
  // starting well past the scratch region.
  let allocCursor = 8192;
  const scratch = new Scratch(
    256,
    1024,
    (size) => {
      const p = allocCursor;
      allocCursor += size;
      return p;
    },
    (_ptr, _size) => {},
  );

  const calls: any[][] = [];
  const exportFn = (...args: any[]) => {
    calls.push(args);
    // Caller passed (a0, statusPtr). Echo a0 back as the scalar return.
    return args[0];
  };

  const ctx: DispatchContext = {
    memory,
    scratch,
    structs: new Map(),
    callbackDefs: new Map(),
    alloc: (size, _align) => {
      const p = allocCursor;
      allocCursor += size;
      return p;
    },
    free: () => {},
    installCallback: () => {
      throw new Error("not used");
    },
    useJit: true,
  };

  const def: FunctionDef = {
    args: [FfiType.UInt32],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  };

  // Pre-poison the status byte so we can verify the JIT body zeros it.
  // The reserved layout starts at the arena base (256) with status at
  // offset 0 and is 32 bytes wide.
  memory.writeU8(256, 0x7f);

  const dispatch = buildJitDispatcher(ctx, exportFn, def, "test_fn");
  assert.ok(dispatch, "buildJitDispatcher returned a function");

  const status = { code: 0xff, errorBuf: undefined as any };
  const result = (dispatch as (...a: any[]) => unknown)(42, status);

  assert.strictEqual(result, 42, "scalar return value forwarded");
  assert.strictEqual(calls.length, 1, "exportFn called exactly once");
  // Wasm-side args: (a0, statusPtr). a0 coerced via `| 0` is still 42.
  assert.strictEqual(calls[0][0], 42);
  assert.strictEqual(typeof calls[0][1], "number");
  // Status code must be re-read from memory (we zeroed it before the call,
  // and exportFn didn't touch it, so it reads back as 0).
  assert.strictEqual(status.code, 0, "status.code reflects the zeroed byte");
});

/**
 * The error path of the JIT test above. The stub `exportFn` writes `code=1`
 * to the status byte and points the errorBuf RustBuffer slot at a fixed
 * payload in wasm memory. The dispatcher must:
 *   - read the non-zero status code into statusObj.code,
 *   - copy the errorBuf payload bytes into a JS-owned Uint8Array on
 *     statusObj.errorBuf (matching what `copyAndFreeRustBuffer` returns),
 *   - return undefined (skipping the sret read on the error path).
 */
test("buildJitDispatcher error path: code != 0 populates statusObj.errorBuf", () => {
  const wasmMem = new WebAssembly.Memory({ initial: 1 });
  const memory = new Memory(wasmMem);
  let allocCursor = 8192;
  const frees: Array<[number, number]> = [];
  const scratch = new Scratch(
    256,
    1024,
    (size) => {
      const p = allocCursor;
      allocCursor += size;
      return p;
    },
    (_ptr, _size) => {},
  );

  // Stash the errorBuf payload at a known location in wasm memory, well
  // outside the scratch region.
  const ERROR_PAYLOAD = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
  const errorPayloadPtr = 16384;
  memory.writeBytes(errorPayloadPtr, ERROR_PAYLOAD);

  // Reserved layout (computed in buildJitDispatcher via computeAndReserveLayout):
  // status at base+0 (32 bytes: code u8 + pad, then RustBuffer at +8).
  // RustBuffer layout: capacity u64 at +0, len u64 at +8, dataPtr u32 at +16.
  const STATUS_BASE = 256;
  const ERRBUF_BASE = STATUS_BASE + 8; // RCS_ERROR_BUF_OFF

  const exportFn = (...args: any[]) => {
    // Last arg is the status pointer. Write code=1 and populate errorBuf.
    const statusPtr = args[args.length - 1];
    memory.writeU8(statusPtr, 1);
    // errorBuf: capacity=4, len=4, dataPtr=errorPayloadPtr.
    memory.writeU64(statusPtr + 8 + 0, 4n);
    memory.writeU64(statusPtr + 8 + 8, 4n);
    memory.writeU32(statusPtr + 8 + 16, errorPayloadPtr);
    // Scalar return is meaningless on the error path; Rust hasn't written
    // a valid value, but we still return *something* so the host call
    // completes normally — the dispatcher must skip the sret read anyway.
    return 0;
  };

  const ctx: DispatchContext = {
    memory,
    scratch,
    structs: new Map(),
    callbackDefs: new Map(),
    alloc: (size, _align) => {
      const p = allocCursor;
      allocCursor += size;
      return p;
    },
    free: (ptr, size, _align) => {
      frees.push([ptr, size]);
    },
    installCallback: () => {
      throw new Error("not used");
    },
    useJit: true,
  };

  const def: FunctionDef = {
    args: [FfiType.UInt32],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  };

  const dispatch = buildJitDispatcher(ctx, exportFn, def, "test_err_fn");
  assert.ok(dispatch, "buildJitDispatcher returned a function");

  const status = { code: 0, errorBuf: undefined as any };
  const result = (dispatch as (...a: any[]) => unknown)(99, status);

  assert.strictEqual(result, undefined, "error path returns undefined");
  assert.strictEqual(status.code, 1, "status.code propagated from wasm memory");
  // copyAndFreeRustBuffer reads `len` bytes from dataPtr into a fresh
  // Uint8Array and frees the wasm allocation.
  assert.ok(
    status.errorBuf instanceof Uint8Array,
    "errorBuf lifted into a Uint8Array",
  );
  assert.strictEqual(
    (status.errorBuf as Uint8Array).byteLength,
    4,
    "errorBuf length matches the wasm-side RustBuffer.len",
  );
  assert.deepStrictEqual(
    Array.from(status.errorBuf as Uint8Array),
    Array.from(ERROR_PAYLOAD),
    "errorBuf payload copied byte-for-byte from wasm memory",
  );
  // The copy path also frees the wasm allocation (capacity bytes).
  assert.deepStrictEqual(
    frees,
    [[errorPayloadPtr, 4]],
    "copyAndFreeRustBuffer freed the errorBuf allocation",
  );
});

test("buildJitDispatcher returns undefined for unsupported (callback) arg", () => {
  const wasmMem = new WebAssembly.Memory({ initial: 1 });
  const memory = new Memory(wasmMem);
  const scratch = new Scratch(
    256,
    1024,
    () => 0,
    () => {},
  );
  const ctx: DispatchContext = {
    memory,
    scratch,
    structs: new Map(),
    callbackDefs: new Map(),
    alloc: () => 0,
    free: () => {},
    installCallback: () => 0,
    useJit: true,
  };
  const def: FunctionDef = {
    args: [FfiType.Callback("cb")],
    ret: FfiType.Void,
    hasRustCallStatus: false,
  };
  const jit = buildJitDispatcher(ctx, () => 0, def, "f");
  assert.strictEqual(jit, undefined);
});

test("registerSync({disableJit:true}) still produces a working dispatcher", async () => {
  const mod = await UniffiNativeModule.open(HOST_BYTES);
  const nm = mod.registerSync(
    {
      symbols: {
        rustbuffer_alloc: "_",
        rustbuffer_free: "_",
        rustbuffer_from_bytes: "_",
      },
      functions: {
        uniffi_test_add: {
          args: [FfiType.Int32, FfiType.Int32],
          ret: FfiType.Int32,
          hasRustCallStatus: true,
        },
      },
      callbacks: {},
      structs: {},
    },
    { disableJit: true },
  );
  const status = { code: 0 };
  const result = nm.uniffi_test_add(5, 6, status);
  assert.strictEqual(result, 11);
  assert.strictEqual(status.code, 0);
});

// --- ForeignBytes (`&[u8]`) arguments ----------------------------------------
//
// On wasm32 the C ABI passes the 8-byte `ForeignBytes { len: i32, data: *const
// u8 }` struct by pointer, so the dispatcher must reserve a scratch slot for it
// and hand the callee that slot's address. The payload bytes are JavaScrip-
// owned, so the dispatcher copies them into wasm memory and frees the copy
// after the call (Rust only borrows them).

interface ForeignBytesHarness {
  ctx: DispatchContext;
  allocs: Array<[number, number]>;
  frees: Array<[number, number, number]>;
  // Shared, ordered log of what happened during a dispatch, so a test can
  // assert that the callee read the payload *before* cleanup freed it.
  events: string[];
  // Reserve arena base + size, so tests can assert the struct lands in scratch.
  arenaBase: number;
  arenaSize: number;
}

/**
 * A `DispatchContext` backed by a real `WebAssembly.Memory` and a bump
 * allocator that records every alloc/free, starting well past the scratch
 * region so payloads never collide with the reserved slot.
 */
function foreignBytesHarness(): ForeignBytesHarness {
  const wasmMem = new WebAssembly.Memory({ initial: 1 });
  const memory = new Memory(wasmMem);
  const allocs: Array<[number, number]> = [];
  const frees: Array<[number, number, number]> = [];
  const events: string[] = [];
  let cursor = 8192;
  const arenaBase = 256;
  const arenaSize = 1024;
  const scratch = new Scratch(
    arenaBase,
    arenaSize,
    () => 0,
    () => {},
  );
  const ctx: DispatchContext = {
    memory,
    scratch,
    structs: new Map(),
    callbackDefs: new Map(),
    alloc: (size, align) => {
      allocs.push([size, align]);
      const p = cursor;
      cursor += size;
      return p;
    },
    free: (ptr, size, align) => {
      frees.push([ptr, size, align]);
      events.push(`free:${ptr}:${size}`);
    },
    installCallback: () => {
      throw new Error("not used");
    },
    useJit: false, // exercise the interpreted `planArg` path
  };
  return { ctx, allocs, frees, events, arenaBase, arenaSize };
}

const FOREIGN_BYTES_DEF: FunctionDef = {
  args: [FfiType.ForeignBytes],
  ret: FfiType.UInt32,
  hasRustCallStatus: true,
};

test("ForeignBytes arg: subarray window is copied, then freed after the call", () => {
  const h = foreignBytesHarness();
  const memory = h.ctx.memory;
  const backing = new Uint8Array([201, 202, 1, 2, 203]);
  const view = backing.subarray(2, 4); // [1, 2]

  let structPtr = -1;
  let dataPtr = -1;
  let observedLen = -1;
  let observedBytes: number[] = [];
  const exportFn = (...args: any[]) => {
    structPtr = args[0];
    observedLen = memory.readI32(args[0]);
    dataPtr = memory.readU32(args[0] + 4);
    observedBytes = [];
    for (let i = 0; i < observedLen; i++) {
      observedBytes.push(memory.readU8(dataPtr + i));
    }
    h.events.push(`read:${observedBytes.join(",")}`);
    const statusPtr = args[args.length - 1];
    memory.writeU8(statusPtr, 0);
    return observedLen;
  };

  const dispatch = specializeFunction(
    h.ctx,
    exportFn,
    FOREIGN_BYTES_DEF,
    "fb_subarray",
  );
  const status = { code: 0xff };
  const result = dispatch(view, status);

  assert.strictEqual(
    observedLen,
    2,
    "len is the view byteLength, not the backing length",
  );
  assert.deepStrictEqual(
    observedBytes,
    [1, 2],
    "only the subarray window is copied",
  );
  assert.strictEqual(result, 2);
  assert.strictEqual(status.code, 0);
  assert.ok(
    structPtr >= h.arenaBase && structPtr < h.arenaBase + h.arenaSize,
    "ForeignBytes struct is written into the reserved scratch region",
  );
  assert.deepStrictEqual(
    h.allocs,
    [[2, 1]],
    "allocated exactly the payload bytes",
  );
  // The allocation is released only after the call has read the bytes. Both
  // are recorded in one ordered log, so the ordering is directly observed.
  assert.deepStrictEqual(
    h.events,
    [`read:1,2`, `free:${dataPtr}:2`],
    "callee reads the payload before cleanup frees it",
  );
  assert.deepStrictEqual(
    h.frees,
    [[dataPtr, 2, 1]],
    "payload freed after the call, with the exact (ptr, len)",
  );
});

test("ForeignBytes arg: empty buffer arrives as (len 0, null) with no alloc/free", () => {
  const h = foreignBytesHarness();
  const memory = h.ctx.memory;

  let observedLen = -1;
  let observedDataPtr = -1;
  const exportFn = (...args: any[]) => {
    observedLen = memory.readI32(args[0]);
    observedDataPtr = memory.readU32(args[0] + 4);
    const statusPtr = args[args.length - 1];
    memory.writeU8(statusPtr, 0);
    return 0;
  };

  const dispatch = specializeFunction(
    h.ctx,
    exportFn,
    FOREIGN_BYTES_DEF,
    "fb_empty",
  );
  const status = { code: 0 };
  dispatch(new Uint8Array(0), status);

  assert.strictEqual(observedLen, 0, "zero-length buffer reports len 0");
  assert.strictEqual(
    observedDataPtr,
    0,
    "zero-length buffer passes a null pointer",
  );
  assert.deepStrictEqual(h.allocs, [], "no payload allocation for empty input");
  assert.deepStrictEqual(h.frees, [], "nothing to free for empty input");
});

test("ForeignBytes arg: multiple args each copy and free independently", () => {
  const h = foreignBytesHarness();
  const memory = h.ctx.memory;

  const seen: Array<[number, number[]]> = [];
  const structPtrs: number[] = [];
  const payloadPtrs: number[] = [];
  const exportFn = (...args: any[]) => {
    seen.length = 0;
    structPtrs.length = 0;
    payloadPtrs.length = 0;
    // Two ForeignBytes args, then the status pointer.
    for (let a = 0; a < 2; a++) {
      structPtrs.push(args[a]);
      const len = memory.readI32(args[a]);
      const dataPtr = memory.readU32(args[a] + 4);
      payloadPtrs.push(dataPtr);
      const bytes: number[] = [];
      for (let i = 0; i < len; i++) bytes.push(memory.readU8(dataPtr + i));
      seen.push([len, bytes]);
    }
    const statusPtr = args[args.length - 1];
    memory.writeU8(statusPtr, 0);
    return 0;
  };

  const def: FunctionDef = {
    args: [FfiType.ForeignBytes, FfiType.ForeignBytes],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  };
  const dispatch = specializeFunction(h.ctx, exportFn, def, "fb_two");
  dispatch(new Uint8Array([7, 8]), new Uint8Array([9]), { code: 0 });

  assert.deepStrictEqual(seen, [
    [2, [7, 8]],
    [1, [9]],
  ]);
  assert.deepStrictEqual(h.allocs, [
    [2, 1],
    [1, 1],
  ]);
  // Exact (ptr, len) pairs, so a cleanup that freed the wrong pointer or the
  // wrong length would fail here rather than merely keeping the count at 2.
  assert.deepStrictEqual(h.frees, [
    [payloadPtrs[0], 2, 1],
    [payloadPtrs[1], 1, 1],
  ]);
});

test("ForeignBytes arg: payload is freed even when the callee reports an error", () => {
  const h = foreignBytesHarness();
  const memory = h.ctx.memory;

  let observedLen = -1;
  let observedDataPtr = -1;
  const exportFn = (...args: any[]) => {
    observedLen = memory.readI32(args[0]);
    observedDataPtr = memory.readU32(args[0] + 4);
    const statusPtr = args[args.length - 1];
    // code 1 → the dispatcher takes the error path and skips `reg.finish`.
    memory.writeU8(statusPtr, 1);
    return 0;
  };

  const dispatch = specializeFunction(
    h.ctx,
    exportFn,
    FOREIGN_BYTES_DEF,
    "fb_error",
  );
  const status = { code: 0, errorBuf: undefined as any };
  assert.strictEqual(dispatch(new Uint8Array([1, 2, 3]), status), undefined);
  assert.strictEqual(status.code, 1);
  assert.strictEqual(observedLen, 3, "callee saw the payload length");
  assert.deepStrictEqual(h.allocs, [[3, 1]]);
  assert.deepStrictEqual(
    h.frees,
    [[observedDataPtr, 3, 1]],
    "borrowed payload freed on the error path with the exact (ptr, len)",
  );
});

test("buildJitDispatcher declines ForeignBytes args so the interpreted path frees", () => {
  const h = foreignBytesHarness();
  h.ctx.useJit = true;
  const jit = buildJitDispatcher(h.ctx, () => 0, FOREIGN_BYTES_DEF, "fb_jit");
  assert.strictEqual(
    jit,
    undefined,
    "ForeignBytes must fall back to the interpreted dispatcher",
  );

  // ...and `specializeFunction` therefore still produces a working dispatcher,
  // with the borrowed payload freed.
  const memory = h.ctx.memory;
  const exportFn = (...args: any[]) => {
    const statusPtr = args[args.length - 1];
    memory.writeU8(statusPtr, 0);
    return memory.readI32(args[0]);
  };
  const dispatch = specializeFunction(
    h.ctx,
    exportFn,
    FOREIGN_BYTES_DEF,
    "fb_jit_fallback",
  );
  const status = { code: 0xff };
  assert.strictEqual(dispatch(new Uint8Array([1, 2, 3]), status), 3);
  assert.deepStrictEqual(
    h.frees,
    [[8192, 3, 1]],
    "payload freed via the interpreted path, with the exact (ptr, len)",
  );
});

test("ForeignBytes arg: re-entrant call frees each payload exactly once", () => {
  const h = foreignBytesHarness();
  const memory = h.ctx.memory;
  const outer = new Uint8Array([1, 2]);
  const inner = new Uint8Array([3, 4, 5]);

  let inInnerCall = false;
  const dispatchRef: { fn?: (...a: any[]) => any } = {};
  const exportFn = (...args: any[]) => {
    const statusPtr = args[args.length - 1];
    if (!inInnerCall) {
      inInnerCall = true;
      // Re-enter the same export while the outer call is in flight. The inner
      // dispatch reuses the shared reserved scratch slot, overwriting the
      // outer call's ForeignBytes struct.
      dispatchRef.fn!(inner, { code: 0 });
      inInnerCall = false;
    }
    memory.writeU8(statusPtr, 0);
    return 0;
  };

  const dispatch = specializeFunction(
    h.ctx,
    exportFn,
    FOREIGN_BYTES_DEF,
    "fb_reentrant",
  );
  dispatchRef.fn = dispatch;
  dispatch(outer, { code: 0 });

  // Outer payload at 8192 (len 2), inner at 8194 (len 3). Each must be freed
  // exactly once. The old stateless cleanup re-read the shared slot in the
  // outer `finally`, freeing 8194 a second time and leaking 8192.
  assert.deepStrictEqual(h.allocs, [
    [2, 1],
    [3, 1],
  ]);
  assert.deepStrictEqual(
    h.frees,
    [
      [8194, 3, 1],
      [8192, 2, 1],
    ],
    "inner then outer payload, each freed exactly once",
  );
});

test("ForeignBytes arg: non-Uint8Array source throws before allocating", () => {
  const h = foreignBytesHarness();
  const dispatch = specializeFunction(
    h.ctx,
    () => {
      throw new Error("callee must not be reached");
    },
    FOREIGN_BYTES_DEF,
    "fb_bad_type",
  );

  // A plain ArrayBuffer used to reach `Memory.writeBytes` and throw a bare
  // TypeError from `TypedArray.prototype.set` *after* allocating.
  assert.throws(
    () => dispatch(new ArrayBuffer(4) as unknown as Uint8Array, { code: 0 }),
    /expected a Uint8Array/,
  );
  assert.deepStrictEqual(h.allocs, [], "guard ran before any allocation");
});

test("ForeignBytes arg: detached source throws before allocating", () => {
  const h = foreignBytesHarness();
  const backing = new ArrayBuffer(4);
  const view = new Uint8Array(backing);
  // Detach the backing buffer. The view now reports length 0, which without a
  // guard would quietly reach Rust as an empty ForeignBytes.
  structuredClone(view, { transfer: [backing] });

  const dispatch = specializeFunction(
    h.ctx,
    () => {
      throw new Error("callee must not be reached");
    },
    FOREIGN_BYTES_DEF,
    "fb_detached",
  );
  assert.throws(() => dispatch(view, { code: 0 }), /source view is detached/);
  assert.deepStrictEqual(h.allocs, [], "guard ran before any allocation");
});

test("ForeignBytes arg: earlier payload is freed when a later prepare throws", () => {
  const h = foreignBytesHarness();
  const def: FunctionDef = {
    args: [FfiType.ForeignBytes, FfiType.ForeignBytes],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  };
  const dispatch = specializeFunction(
    h.ctx,
    () => {
      throw new Error("callee must not be reached");
    },
    def,
    "fb_second_throws",
  );

  // The second arg is invalid, so its `prepare` throws *after* the first arg
  // already allocated. The `try` must have started before the prepare loop so
  // the first payload is still released.
  assert.throws(
    () =>
      dispatch(new Uint8Array([1, 2]), new ArrayBuffer(4) as any, {
        code: 0,
      }),
    /expected a Uint8Array/,
  );
  assert.deepStrictEqual(h.allocs, [[2, 1]]);
  assert.deepStrictEqual(
    h.frees,
    [[8192, 2, 1]],
    "first payload freed even though a later prepare threw",
  );
});

test("interpreted dispatcher passes bigint (UInt64) scalar args through", () => {
  // Regression guard: `prepare` returns a bare word for scalars, so the
  // dispatcher must not assume every word is a `number` — i64/handle args
  // arrive as `bigint`.
  const wasmMem = new WebAssembly.Memory({ initial: 1 });
  const memory = new Memory(wasmMem);
  const scratch = new Scratch(
    256,
    1024,
    () => 0,
    () => {},
  );
  const ctx: DispatchContext = {
    memory,
    scratch,
    structs: new Map(),
    callbackDefs: new Map(),
    alloc: () => 0,
    free: () => {},
    installCallback: () => 0,
    useJit: false,
  };
  const seen: any[] = [];
  const exportFn = (...args: any[]) => {
    seen.push(args[0]);
    memory.writeU8(args[args.length - 1], 0);
    return 0;
  };
  const def: FunctionDef = {
    args: [FfiType.UInt64],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  };
  const dispatch = specializeFunction(ctx, exportFn, def, "u64_arg");
  dispatch(123n, { code: 0 });
  assert.deepStrictEqual(seen, [123n]);
});
