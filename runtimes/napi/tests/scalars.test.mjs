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

test("register and call i32 add function", () => {
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
  const result = nm.uniffi_test_fn_add(3, 4, status);
  assert.strictEqual(result, 7);
  assert.strictEqual(status.code, 0);
});

test("register and call i8 negate function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_negate: {
        args: [FfiType.Int8],
        ret: FfiType.Int8,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_negate(42, status);
  assert.strictEqual(result, -42);
});

test("register and call u64 handle function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_handle: {
        args: [],
        ret: FfiType.Handle,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_handle(status);
  assert.strictEqual(result, 0xdeadbeef12345678n);
});

test("register and call void function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_void: {
        args: [],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_void(status);
  assert.strictEqual(result, undefined);
  assert.strictEqual(status.code, 0);
});

test("register and call f64 double function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_double: {
        args: [FfiType.Float64],
        ret: FfiType.Float64,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_double(3.14, status);
  assert.strictEqual(result, 6.28);
});

test("register and call f32 half function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_float32_half: {
        args: [FfiType.Float32],
        ret: FfiType.Float32,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_float32_half(6.0, status);
  assert.strictEqual(status.code, 0);
  assert.ok(Math.abs(result - 3.0) < 0.001);
});

test("register and call i16 negate function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_i16_negate: {
        args: [FfiType.Int16],
        ret: FfiType.Int16,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_i16_negate(1234, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, -1234);
});

test("register and call u16 double function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_u16_double: {
        args: [FfiType.UInt16],
        ret: FfiType.UInt16,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_u16_double(300, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 600);
});

test("register and call i64 negate function", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_i64_negate: {
        args: [FfiType.Int64],
        ret: FfiType.Int64,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_i64_negate(9007199254740993n, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, -9007199254740993n);
});
