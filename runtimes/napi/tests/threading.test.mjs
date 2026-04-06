import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

const LIB_PATH = libPath("uniffi_napi_test_lib");

const SYMBOLS = {
  rustbufferAlloc: "uniffi_test_rustbuffer_alloc",
  rustbufferFree: "uniffi_test_rustbuffer_free",
  rustbufferFromBytes: "uniffi_test_rustbuffer_from_bytes",
};

function openLib() {
  return UniffiNativeModule.open(LIB_PATH);
}

test("callback: invoked from another thread dispatches to event loop", async () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      simple_callback: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_call_callback_from_thread: {
        args: [
          FfiType.Callback("simple_callback"),
          FfiType.UInt64,
          FfiType.Int8,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  const result = await new Promise((resolve) => {
    const callback = (handle, value) => {
      resolve({ handle, value });
    };

    const status = { code: 0 };
    nm.uniffi_test_fn_call_callback_from_thread(callback, 99n, -3, status);
    assert.strictEqual(status.code, 0);
  });

  assert.strictEqual(result.handle, 99n);
  assert.strictEqual(result.value, -3);
});

test("callback: receives RustBuffer arg from another thread", async () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      buffer_callback: {
        args: [FfiType.UInt64, FfiType.RustBuffer],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_call_callback_with_buffer_from_thread: {
        args: [FfiType.Callback("buffer_callback"), FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  const result = await new Promise((resolve, reject) => {
    const callback = (handle, data) => {
      resolve({ handle, data });
    };
    const status = { code: 0 };
    nm.uniffi_test_fn_call_callback_with_buffer_from_thread(
      callback,
      99n,
      status,
    );
    assert.strictEqual(status.code, 0);
    const timer = setTimeout(() => reject(new Error("Timed out")), 5000);
    timer.unref();
  });

  assert.strictEqual(result.handle, 99n);
  assert.ok(result.data instanceof Uint8Array);
  assert.deepStrictEqual(result.data, new Uint8Array([0xca, 0xfe, 0xba, 0xbe]));
});
