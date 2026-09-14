/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// A worker registers and is torn down, then the main thread registers and invokes a callback.
// Run as a script by threading.test.mjs; see there for what it is guarding.
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

const open = () =>
  UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
    DEFINITIONS,
  );

// The worker registers before the main thread does, so it is the one that installs a cleanup hook
// when only the first environment gets one. Its teardown is what must not affect the main thread.
if (!isMainThread) {
  open();
} else {
  const worker = new Worker(new URL(import.meta.url));
  await new Promise((resolve) => worker.on("exit", resolve));

  const nm = open();
  const status = { code: 0 };
  nm.uniffi_test_fn_call_callback(
    (handle, value) => console.log(`CALLED ${handle} ${value}`),
    7n,
    42,
    status,
  );
  if (status.code !== 0) throw new Error(`call status ${status.code}`);
}
