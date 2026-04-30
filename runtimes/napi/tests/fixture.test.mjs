/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import {
  lowerString,
  liftString,
  liftArithmeticError,
} from "./helpers/converters.mjs";
import { continuationCallback, uniffiRustCallAsync } from "./helpers/async.mjs";
import { libPath } from "./helpers/lib-path.mjs";

const LIB_PATH = libPath("uniffi_fixture_simple");

const CRATE = "uniffi_fixture_simple";

const SYMBOLS = {
  rustbufferAlloc: `ffi_${CRATE}_rustbuffer_alloc`,
  rustbufferFree: `ffi_${CRATE}_rustbuffer_free`,
  rustbufferFromBytes: `ffi_${CRATE}_rustbuffer_from_bytes`,
};

function openAndRegister(
  extraFunctions = {},
  extraCallbacks = {},
  extraStructs = {},
) {
  const mod = UniffiNativeModule.open(LIB_PATH);
  return mod.register({
    symbols: SYMBOLS,
    structs: extraStructs,
    callbacks: extraCallbacks,
    functions: extraFunctions,
  });
}

test('fixture: greet("World") = "Hello, World!" (sync string)', () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_greet`]: {
      args: [FfiType.RustBuffer],
      ret: FfiType.RustBuffer,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_greet`](
    lowerString("World"),
    status,
  );
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liftString(result), "Hello, World!");
});

test("fixture: add(3, 4) = 7 (sync scalar)", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_add`]: {
      args: [FfiType.UInt32, FfiType.UInt32],
      ret: FfiType.UInt32,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_add`](3, 4, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 7);
});

test("fixture: divide(1.0, 0.0) returns error (sync error path)", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_divide`]: {
      args: [FfiType.Float64, FfiType.Float64],
      ret: FfiType.Float64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  nm[`uniffi_${CRATE}_fn_func_divide`](1.0, 0.0, status);
  assert.notStrictEqual(status.code, 0, "Expected non-zero error code");
  assert.ok(status.errorBuf instanceof Uint8Array, "Expected errorBuf");

  const error = liftArithmeticError(status.errorBuf);
  assert.strictEqual(error.variant, 1); // DivisionByZero
  assert.ok(error.reason.includes("cannot divide by zero"));
});

test("fixture: divide(10.0, 2.0) = 5.0 (sync success path)", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_divide`]: {
      args: [FfiType.Float64, FfiType.Float64],
      ret: FfiType.Float64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_divide`](10.0, 2.0, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 5.0);
});

test("fixture: async_add(3, 4) = 7 (async scalar)", async () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_func_async_add`]: {
        args: [FfiType.UInt32, FfiType.UInt32],
        ret: FfiType.Handle,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_poll_u32`]: {
        args: [
          FfiType.Handle,
          FfiType.Callback("rust_future_continuation"),
          FfiType.UInt64,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`ffi_${CRATE}_rust_future_complete_u32`]: {
        args: [FfiType.Handle],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_free_u32`]: {
        args: [FfiType.Handle],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    {
      rust_future_continuation: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
  );

  const result = await uniffiRustCallAsync(nm, {
    rustFutureFunc: () => {
      const status = { code: 0 };
      return nm[`uniffi_${CRATE}_fn_func_async_add`](3, 4, status);
    },
    pollFunc: `ffi_${CRATE}_rust_future_poll_u32`,
    completeFunc: `ffi_${CRATE}_rust_future_complete_u32`,
    freeFunc: `ffi_${CRATE}_rust_future_free_u32`,
  });

  assert.strictEqual(result, 7);
});

// Shared Calculator VTable registration definitions.
//
// The UniFFI 0.31 VTable struct for Calculator has this C layout:
//   { uniffi_free, uniffi_clone, add, concatenate }
// Each method callback uses the "out-return" convention: the return value is
// written through an out-pointer argument, and the C function itself returns void.
const CALCULATOR_CALLBACKS = {
  callback_calculator_free: {
    args: [FfiType.UInt64],
    ret: FfiType.Void,
    hasRustCallStatus: false,
  },
  callback_calculator_clone: {
    args: [FfiType.UInt64],
    ret: FfiType.UInt64,
    hasRustCallStatus: false,
  },
  callback_calculator_add: {
    args: [FfiType.UInt64, FfiType.UInt32, FfiType.UInt32],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
    outReturn: true,
  },
  callback_calculator_concatenate: {
    args: [FfiType.UInt64, FfiType.RustBuffer, FfiType.RustBuffer],
    ret: FfiType.RustBuffer,
    hasRustCallStatus: true,
    outReturn: true,
  },
};

const CALCULATOR_STRUCT = {
  VTable_Calculator: [
    { name: "uniffi_free", type: FfiType.Callback("callback_calculator_free") },
    {
      name: "uniffi_clone",
      type: FfiType.Callback("callback_calculator_clone"),
    },
    { name: "add", type: FfiType.Callback("callback_calculator_add") },
    {
      name: "concatenate",
      type: FfiType.Callback("callback_calculator_concatenate"),
    },
  ],
};

const CALCULATOR_VTABLE_JS = {
  uniffi_free: (handle) => {},
  uniffi_clone: (handle) => handle,
  add: (handle, a, b) => {
    return { code: 0, pointee: a + b };
  },
  concatenate: (handle, aBuf, bBuf) => {
    return { code: 0, pointee: lowerString(liftString(aBuf) + liftString(bBuf)) };
  },
};

test("fixture: Calculator.add via VTable (sync callback, scalar)", () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_init_callback_vtable_calculator`]: {
        args: [FfiType.Reference(FfiType.Struct("VTable_Calculator"))],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`uniffi_${CRATE}_fn_func_use_calculator`]: {
        args: [FfiType.UInt64, FfiType.UInt32, FfiType.UInt32],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
    },
    CALCULATOR_CALLBACKS,
    CALCULATOR_STRUCT,
  );

  // Register VTable
  nm[`uniffi_${CRATE}_fn_init_callback_vtable_calculator`](
    CALCULATOR_VTABLE_JS,
  );

  // Call use_calculator(calc_handle, 3, 4) => should invoke add(3,4) => 7
  const status2 = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_use_calculator`](
    1n,
    3,
    4,
    status2,
  );
  assert.strictEqual(status2.code, 0);
  assert.strictEqual(result, 7);
});

test("fixture: Calculator.concatenate via VTable (sync callback, string)", () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_init_callback_vtable_calculator`]: {
        args: [FfiType.Reference(FfiType.Struct("VTable_Calculator"))],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`uniffi_${CRATE}_fn_func_use_calculator_strings`]: {
        args: [FfiType.UInt64, FfiType.RustBuffer, FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
    CALCULATOR_CALLBACKS,
    CALCULATOR_STRUCT,
  );

  // Register VTable
  nm[`uniffi_${CRATE}_fn_init_callback_vtable_calculator`](
    CALCULATOR_VTABLE_JS,
  );

  // Call use_calculator_strings(calc_handle, "Hello, ", "World!") => "Hello, World!"
  const status2 = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_use_calculator_strings`](
    1n,
    lowerString("Hello, "),
    lowerString("World!"),
    status2,
  );
  assert.strictEqual(status2.code, 0);
  assert.strictEqual(liftString(result), "Hello, World!");
});

test("fixture: Calculator.add from background thread (async + cross-thread VTable)", async () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_init_callback_vtable_calculator`]: {
        args: [FfiType.Reference(FfiType.Struct("VTable_Calculator"))],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`uniffi_${CRATE}_fn_func_use_calculator_from_thread`]: {
        args: [FfiType.UInt64, FfiType.UInt32, FfiType.UInt32],
        ret: FfiType.Handle,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_poll_u32`]: {
        args: [
          FfiType.Handle,
          FfiType.Callback("rust_future_continuation"),
          FfiType.UInt64,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`ffi_${CRATE}_rust_future_complete_u32`]: {
        args: [FfiType.Handle],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_free_u32`]: {
        args: [FfiType.Handle],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    {
      ...CALCULATOR_CALLBACKS,
      rust_future_continuation: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    CALCULATOR_STRUCT,
  );

  // Register the Calculator VTable so Rust can call back into JS
  nm[`uniffi_${CRATE}_fn_init_callback_vtable_calculator`](
    CALCULATOR_VTABLE_JS,
  );

  // Call use_calculator_from_thread(calc_handle, 3, 4) — async, spawns a blocking
  // thread that invokes calc.add() via the VTable, dispatched back to JS via TSF.
  const result = await uniffiRustCallAsync(nm, {
    rustFutureFunc: () => {
      const status = { code: 0 };
      return nm[`uniffi_${CRATE}_fn_func_use_calculator_from_thread`](
        1n,
        3,
        4,
        status,
      );
    },
    pollFunc: `ffi_${CRATE}_rust_future_poll_u32`,
    completeFunc: `ffi_${CRATE}_rust_future_complete_u32`,
    freeFunc: `ffi_${CRATE}_rust_future_free_u32`,
  });

  assert.strictEqual(result, 7);
});

test('fixture: async_greet("World") = "Hello, World!" (async string)', async () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_func_async_greet`]: {
        args: [FfiType.RustBuffer],
        ret: FfiType.Handle,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_poll_rust_buffer`]: {
        args: [
          FfiType.Handle,
          FfiType.Callback("rust_future_continuation"),
          FfiType.UInt64,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`ffi_${CRATE}_rust_future_complete_rust_buffer`]: {
        args: [FfiType.Handle],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_free_rust_buffer`]: {
        args: [FfiType.Handle],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    {
      rust_future_continuation: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
  );

  const result = await uniffiRustCallAsync(nm, {
    rustFutureFunc: () => {
      const status = { code: 0 };
      return nm[`uniffi_${CRATE}_fn_func_async_greet`](
        lowerString("World"),
        status,
      );
    },
    pollFunc: `ffi_${CRATE}_rust_future_poll_rust_buffer`,
    completeFunc: `ffi_${CRATE}_rust_future_complete_rust_buffer`,
    freeFunc: `ffi_${CRATE}_rust_future_free_rust_buffer`,
    liftFunc: liftString,
  });

  assert.strictEqual(result, "Hello, World!");
});

test("fixture: register with struct-by-value does not crash", () => {
  const mod = UniffiNativeModule.open(LIB_PATH);
  assert.doesNotThrow(() => {
    mod.register({
      symbols: SYMBOLS,
      functions: {},
      callbacks: {
        test_cb: {
          args: [FfiType.Struct("TestResult")],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      structs: {
        TestResult: [
          { name: "value", type: FfiType.UInt32 },
          { name: "code", type: FfiType.Int8 },
        ],
      },
    });
  });
});

test("fixture: AsyncFetcher.fetch (async foreign trait)", async () => {
  const nm = openAndRegister(
    {
      [`uniffi_${CRATE}_fn_init_callback_vtable_asyncfetcher`]: {
        args: [FfiType.Reference(FfiType.Struct("VTable_AsyncFetcher"))],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`uniffi_${CRATE}_fn_func_use_async_fetcher`]: {
        args: [FfiType.Handle, FfiType.RustBuffer],
        ret: FfiType.Handle,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_poll_rust_buffer`]: {
        args: [
          FfiType.Handle,
          FfiType.Callback("rust_future_continuation"),
          FfiType.UInt64,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      [`ffi_${CRATE}_rust_future_complete_rust_buffer`]: {
        args: [FfiType.Handle],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
      [`ffi_${CRATE}_rust_future_free_rust_buffer`]: {
        args: [FfiType.Handle],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    {
      rust_future_continuation: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      callback_asyncfetcher_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      callback_asyncfetcher_clone: {
        args: [FfiType.UInt64],
        ret: FfiType.UInt64,
        hasRustCallStatus: false,
      },
      callback_asyncfetcher_fetch: {
        args: [
          FfiType.UInt64,
          FfiType.RustBuffer,
          FfiType.Callback("ForeignFutureCompleteRustBuffer"),
          FfiType.UInt64,
          FfiType.MutReference(
            FfiType.Struct("ForeignFutureDroppedCallbackStruct"),
          ),
        ],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      ForeignFutureCompleteRustBuffer: {
        args: [FfiType.UInt64, FfiType.Struct("ForeignFutureResultRustBuffer")],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
      ForeignFutureDroppedCallback: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    {
      VTable_AsyncFetcher: [
        {
          name: "uniffi_free",
          type: FfiType.Callback("callback_asyncfetcher_free"),
        },
        {
          name: "uniffi_clone",
          type: FfiType.Callback("callback_asyncfetcher_clone"),
        },
        {
          name: "fetch",
          type: FfiType.Callback("callback_asyncfetcher_fetch"),
        },
      ],
      ForeignFutureResultRustBuffer: [
        { name: "returnValue", type: FfiType.RustBuffer },
        { name: "callStatus", type: FfiType.Struct("RustCallStatus") },
      ],
      RustCallStatus: [
        { name: "code", type: FfiType.Int8 },
        { name: "errorBuf", type: FfiType.RustBuffer },
      ],
      ForeignFutureDroppedCallbackStruct: [
        { name: "callbackData", type: FfiType.UInt64 },
        {
          name: "callback",
          type: FfiType.Callback("ForeignFutureDroppedCallback"),
        },
      ],
    },
  );

  // Register the AsyncFetcher VTable
  nm[`uniffi_${CRATE}_fn_init_callback_vtable_asyncfetcher`]({
    uniffi_free: (handle) => {},
    uniffi_clone: (handle) => handle,
    fetch: (handle, inputBuf, completeCb, completeCbData, outDroppedCb) => {
      // Implement the async fetch: prepend "fetched: " to the input string
      const input = liftString(inputBuf);
      const result = `fetched: ${input}`;

      // Call the completion callback with the result
      completeCb(completeCbData, {
        returnValue: lowerString(result),
        callStatus: { code: 0, errorBuf: new Uint8Array(0) },
      });
    },
  });

  // Call use_async_fetcher — this is an async function, so poll it
  const result = await uniffiRustCallAsync(nm, {
    rustFutureFunc: () => {
      const status = { code: 0 };
      return nm[`uniffi_${CRATE}_fn_func_use_async_fetcher`](
        1n,
        lowerString("hello"),
        status,
      );
    },
    pollFunc: `ffi_${CRATE}_rust_future_poll_rust_buffer`,
    completeFunc: `ffi_${CRATE}_rust_future_complete_rust_buffer`,
    freeFunc: `ffi_${CRATE}_rust_future_free_rust_buffer`,
    liftFunc: liftString,
  });

  assert.strictEqual(result, "fetched: hello");
});
