/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
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

test("RustBuffer echo: pass Uint8Array, get same bytes back", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_echo_buffer: {
        args: [FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });

  const input = new Uint8Array([1, 2, 3, 4, 5]);
  const status = { code: 0 };
  const result = nm.uniffi_test_fn_echo_buffer(input, status);

  assert.strictEqual(status.code, 0);
  assert.ok(result instanceof Uint8Array);
  assert.deepStrictEqual(result, input);
});

test("RustBuffer echo: empty buffer", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_echo_buffer: {
        args: [FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });

  const input = new Uint8Array([]);
  const status = { code: 0 };
  const result = nm.uniffi_test_fn_echo_buffer(input, status);

  assert.strictEqual(status.code, 0);
  assert.ok(result instanceof Uint8Array);
  assert.strictEqual(result.length, 0);
});

test("RustBuffer: large buffer round-trip (1MB)", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_echo_buffer: {
        args: [FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });

  const input = new Uint8Array(1024 * 1024);
  for (let i = 0; i < input.length; i++) input[i] = i & 0xff;
  const status = { code: 0 };
  const result = nm.uniffi_test_fn_echo_buffer(input, status);

  assert.strictEqual(status.code, 0);
  assert.strictEqual(result.length, input.length);
  assert.deepStrictEqual(result, input);
});

test("RustBuffer: concat two buffers", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_concat_buffers: {
        args: [FfiType.RustBuffer, FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });

  const a = new Uint8Array([1, 2, 3]);
  const b = new Uint8Array([4, 5]);
  const status = { code: 0 };
  const result = nm.uniffi_test_fn_concat_buffers(a, b, status);

  assert.strictEqual(status.code, 0);
  assert.deepStrictEqual(result, new Uint8Array([1, 2, 3, 4, 5]));
});

test("RustBuffer: buffer_len returns correct length", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_buffer_len: {
        args: [FfiType.RustBuffer],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
    },
  });

  const input = new Uint8Array([10, 20, 30, 40]);
  const status = { code: 0 };
  const result = nm.uniffi_test_fn_buffer_len(input, status);

  assert.strictEqual(status.code, 0);
  assert.strictEqual(result, 4);
});

test("RustBuffer: make_buffer creates filled buffer", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      uniffi_test_fn_make_buffer: {
        args: [FfiType.UInt8, FfiType.UInt32],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
  });

  const status = { code: 0 };
  const result = nm.uniffi_test_fn_make_buffer(0xab, 5, status);

  assert.strictEqual(status.code, 0);
  assert.deepStrictEqual(
    result,
    new Uint8Array([0xab, 0xab, 0xab, 0xab, 0xab]),
  );
});
