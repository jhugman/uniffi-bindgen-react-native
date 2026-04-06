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

test("RustCallStatus: error code and errorBuf are written back", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_error: {
        args: [],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  nm.uniffi_test_fn_error(status);

  assert.strictEqual(status.code, 2); // CALL_UNEXPECTED_ERROR
  assert.ok(status.errorBuf instanceof Uint8Array);
  assert.strictEqual(status.errorBuf.length, 20); // "something went wrong"
  const msg = new TextDecoder().decode(status.errorBuf);
  assert.strictEqual(msg, "something went wrong");
});

test("RustCallStatus: success has no errorBuf", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_add: {
        args: [FfiType.Int32, FfiType.Int32],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  nm.uniffi_test_fn_add(1, 2, status);

  assert.strictEqual(status.code, 0);
  assert.strictEqual(status.errorBuf, undefined);
});
