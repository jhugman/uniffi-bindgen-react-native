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

const FUNCTIONS = {
  uniffi_test_fn_add: {
    args: [FfiType.Int32, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: true,
  },
};

function openAndRegister() {
  const nativeModule = UniffiNativeModule.open(LIB_PATH);
  const bindings = nativeModule.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: FUNCTIONS,
  });
  return { nativeModule, bindings };
}

test("unload after successful call — subsequent calls throw", () => {
  const { nativeModule, bindings } = openAndRegister();
  // Sanity: one successful call
  const status = { code: 0 };
  const r = bindings.uniffi_test_fn_add(1, 2, status);
  assert.strictEqual(r, 3);
  nativeModule.unload();
  assert.throws(() => {
    bindings.uniffi_test_fn_add(1, 2, { code: 0 });
  }, /module is unloading or unloaded/i);
});

test("re-register after unload works", () => {
  const { nativeModule: nm1 } = openAndRegister();
  nm1.unload();
  const { nativeModule: nm2, bindings: b2 } = openAndRegister();
  const status = { code: 0 };
  assert.strictEqual(b2.uniffi_test_fn_add(4, 5, status), 9);
  nm2.unload();
});

test("unload with force on a library with no background threads", () => {
  const { nativeModule } = openAndRegister();
  nativeModule.unload({ force: true });
  // Process should not crash.
});

test("unload is idempotent", () => {
  const { nativeModule } = openAndRegister();
  nativeModule.unload();
  nativeModule.unload(); // should not throw
});
