/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Registers on the main thread, then invokes a callback from a worker, printing what it received.
// Run as a script by threading.test.mjs; see there for why it is a separate process.
import { Worker, isMainThread } from "node:worker_threads";

import lib from "../../lib.js";
import { libPath } from "./lib-path.mjs";

const { UniffiNativeModule, FfiType } = lib;

const DEFINITIONS = {
  symbols: {
    rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
    rustbuffer_free: "uniffi_test_rustbuffer_free",
    rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
  },
  structs: {},
  callbacks: {
    simple_callback: {
      args: [FfiType.UInt64, FfiType.Int8],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
  },
  functions: {
    uniffi_test_fn_call_callback: {
      args: [FfiType.Callback("simple_callback"), FfiType.UInt64, FfiType.Int8],
      ret: FfiType.Void,
      hasRustCallStatus: true,
    },
  },
};

const nm = UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
  DEFINITIONS,
);

// The main thread registers first: whichever thread loads the addon first is the one the old code
// treated as "the" JS thread, so a worker that got there first would prove nothing.
if (isMainThread) {
  new Worker(new URL(import.meta.url));
} else {
  const status = { code: 0 };
  nm.uniffi_test_fn_call_callback(
    (handle, value) => console.log(`CALLED ${handle} ${value}`),
    7n,
    42,
    status,
  );
  if (status.code !== 0) throw new Error(`call status ${status.code}`);
}
