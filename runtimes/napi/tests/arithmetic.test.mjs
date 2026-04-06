// Test that the napi player can load and call the arithmetic example crate.
//
// This proves the player works with a real uniffi crate, not just bespoke
// test fixtures. The arithmetic crate exposes: add, sub, div, equal — all
// taking u64 args. add and sub return Result<u64, ArithmeticError>.

import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

const LIB_PATH = libPath("arithmetical");

const CRATE = "arithmetical";

const SYMBOLS = {
  rustbufferAlloc: `ffi_${CRATE}_rustbuffer_alloc`,
  rustbufferFree: `ffi_${CRATE}_rustbuffer_free`,
  rustbufferFromBytes: `ffi_${CRATE}_rustbuffer_from_bytes`,
};

function openAndRegister(functions = {}) {
  const mod = UniffiNativeModule.open(LIB_PATH);
  return mod.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions,
  });
}

const decoder = new TextDecoder();

/**
 * Lift an ArithmeticError from a RustCallStatus errorBuf.
 * Layout: 4-byte big-endian i32 variant index, then the display string
 * (4-byte big-endian i32 length + UTF-8 bytes).
 */
function liftArithmeticError(buf) {
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const variant = view.getInt32(0, false);
  const msgLen = view.getInt32(4, false);
  const msgBytes = new Uint8Array(buf.buffer, buf.byteOffset + 8, msgLen);
  return { variant, message: decoder.decode(msgBytes) };
}

test("arithmetic: add(3, 4) = 7", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_add`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_add`](3n, 4n, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 7n);
});

test("arithmetic: sub(10, 3) = 7", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_sub`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  const result = nm[`uniffi_${CRATE}_fn_func_sub`](10n, 3n, status);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 7n);
});

test("arithmetic: div(10, 2) = 5", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_div`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: false,
    },
  });

  const result = nm[`uniffi_${CRATE}_fn_func_div`](10n, 2n);
  assert.strictEqual(result, 5n);
});

test("arithmetic: equal(3, 3) = true", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_equal`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.Int8,
      hasRustCallStatus: false,
    },
  });

  const result = nm[`uniffi_${CRATE}_fn_func_equal`](3n, 3n);
  assert.strictEqual(result, 1); // true as i8
});

test("arithmetic: equal(3, 4) = false", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_equal`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.Int8,
      hasRustCallStatus: false,
    },
  });

  const result = nm[`uniffi_${CRATE}_fn_func_equal`](3n, 4n);
  assert.strictEqual(result, 0); // false as i8
});

test("arithmetic: add overflow returns ArithmeticError::IntegerOverflow", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_add`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  nm[`uniffi_${CRATE}_fn_func_add`](
    BigInt("18446744073709551615"), // u64::MAX
    1n,
    status,
  );
  assert.notStrictEqual(status.code, 0, "Expected error status");
  assert.ok(status.errorBuf instanceof Uint8Array, "Expected errorBuf");

  const error = liftArithmeticError(status.errorBuf);
  assert.strictEqual(error.variant, 1); // IntegerOverflow
  assert.ok(error.message.includes("18446744073709551615"));
  assert.ok(error.message.includes("1"));
});

test("arithmetic: sub underflow returns ArithmeticError::IntegerOverflow", () => {
  const nm = openAndRegister({
    [`uniffi_${CRATE}_fn_func_sub`]: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: true,
    },
  });

  const status = { code: 0 };
  nm[`uniffi_${CRATE}_fn_func_sub`](3n, 4n, status);
  assert.notStrictEqual(status.code, 0, "Expected error status");
  assert.ok(status.errorBuf instanceof Uint8Array, "Expected errorBuf");

  const error = liftArithmeticError(status.errorBuf);
  assert.strictEqual(error.variant, 1); // IntegerOverflow
  assert.ok(error.message.includes("3"));
  assert.ok(error.message.includes("4"));
});
