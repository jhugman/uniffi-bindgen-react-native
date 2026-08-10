/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import { spawn } from "node:child_process";
import { join } from "node:path";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { libPath } from "./helpers/lib-path.mjs";

const LIB_PATH = libPath("uniffi_napi_test_lib");

const SYMBOLS = {
  rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
  rustbuffer_free: "uniffi_test_rustbuffer_free",
  rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
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

// A second JS thread can use the runtime: it registers a callback and Rust invokes it.
//
// Run in a child process on purpose. A regression parks a JS thread inside native code, where
// `worker.terminate()` never resolves and `process.exit()` never takes effect, so an in-process
// version would wedge the suite instead of failing it.
test("threads: a second JS thread can invoke a callback", async () => {
  const script = join(
    import.meta.dirname,
    "helpers",
    "second-thread-callback.mjs",
  );
  const child = spawn(process.execPath, [script], {
    stdio: ["ignore", "pipe", "inherit"],
  });

  let out = "";
  child.stdout.on("data", (chunk) => {
    out += String(chunk);
  });

  const exitCode = await new Promise((resolve) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve(null);
    }, 30_000);
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve(code);
    });
  });

  assert.match(out, /^CALLED 7 42$/m, "the worker's callback did not run");
  assert.strictEqual(exitCode, 0, "the child did not exit on its own");
});

// One environment's teardown does not disturb another's.
//
// The worker registers first, so under a single process-wide cleanup hook it is the environment
// that installs it, and its teardown aborts every environment's ThreadsafeFunctions and marks
// every environment as shutting down. The main thread's callbacks then bail out early and zero
// their return buffers, which looks like a callback that silently never ran.
test("threads: one environment's teardown leaves another's callbacks working", async () => {
  const script = join(import.meta.dirname, "helpers", "env-teardown.mjs");
  const child = spawn(process.execPath, [script], {
    stdio: ["ignore", "pipe", "inherit"],
  });

  let out = "";
  child.stdout.on("data", (chunk) => {
    out += String(chunk);
  });

  const exitCode = await new Promise((resolve) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve(null);
    }, 30_000);
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve(code);
    });
  });

  assert.match(out, /^CALLED 7 42$/m, "the main thread's callback did not run");
  assert.strictEqual(exitCode, 0, "the child did not exit on its own");
});
