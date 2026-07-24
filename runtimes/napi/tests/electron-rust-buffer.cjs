/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */
const assert = require("node:assert/strict");
const { app } = require("electron");
const lib = require("../lib.js");

const { FfiType, UniffiNativeModule } = lib;

void (async () => {
  try {
    const { libPath } = await import("./helpers/lib-path.mjs");
    await app.whenReady();

    const module = UniffiNativeModule.open(libPath("uniffi_napi_test_lib"));
    const native = module.register({
      symbols: {
        rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
        rustbuffer_free: "uniffi_test_rustbuffer_free",
        rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
      },
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

    const input = native.rustbuffer_alloc(4);
    input.set([1, 2, 3, 4]);

    const status = { code: 0 };
    const result = native.uniffi_test_fn_echo_buffer(input, status);
    assert.equal(status.code, 0);
    assert.deepEqual(result, new Uint8Array([1, 2, 3, 4]));
    native.rustbuffer_free(input);
    native.rustbuffer_free(result);
    native.rustbuffer_free(result);
    module.unload({ force: true });

    process.exit(0);
  } catch (error) {
    console.error(error);
    process.exit(1);
  }
})();
