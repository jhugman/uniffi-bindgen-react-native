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
