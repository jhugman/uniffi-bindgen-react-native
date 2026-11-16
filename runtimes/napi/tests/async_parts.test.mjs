/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The future exports the native `register` adds for each function with an `async:` def.
//
// This imports index.js, not lib.js, so `register` is the native one: lib.js composes these
// parts into one entry per async function.
import { test } from "node:test";
import assert from "node:assert";
import native from "../index.js";
const { UniffiNativeModule } = native;
import { libPath } from "./helpers/lib-path.mjs";
import {
  asyncDefinitions,
  CANCEL,
  COMPLETE_RUST_BUFFER,
  COMPLETE_U32,
  CONTINUATION,
  FREE,
  POLL,
  pollToReady,
} from "./helpers/test-lib-futures.mjs";

const LIB = libPath("uniffi_napi_test_lib");

const register = (definitions) =>
  UniffiNativeModule.open(LIB).register(definitions);

const nm = register(asyncDefinitions());
const liveFutures = () => nm.uniffi_test_live_future_count({ code: 0 });
const liveBuffers = () => nm.uniffi_test_live_buffer_count({ code: 0 });

async function completeU32(module, future) {
  await pollToReady(module, future);
  const status = { code: 0 };
  const value = module[COMPLETE_U32](future, status);
  module[FREE](future);
  return { value, status };
}

async function completeBuffer(future) {
  await pollToReady(nm, future);
  const status = { code: 0 };
  const view = nm[COMPLETE_RUST_BUFFER](future, status);
  nm[FREE](future);
  assert.strictEqual(status.code, 0);
  return view;
}

test("register exposes each async function's future exports under their own names", async () => {
  for (const name of [POLL, CANCEL, FREE, COMPLETE_U32, COMPLETE_RUST_BUFFER]) {
    assert.strictEqual(typeof nm[name], "function", name);
  }
  const f = nm.uniffi_test_fn_future_u32(7);
  assert.strictEqual(typeof f, "bigint");
  const { value, status } = await completeU32(nm, f);
  assert.strictEqual(value, 7);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveFutures(), 0);
});

test("complete hands back a RustBuffer result as a copy, freeing the Rust side at once", async () => {
  const before = liveBuffers();
  const v = await completeBuffer(nm.uniffi_test_fn_future_buffer(16));
  assert.strictEqual(liveBuffers(), before);
  nm.rustbuffer_free(v);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(v.byteLength, 16);
  assert.strictEqual(v[15], 15);
  assert.strictEqual(liveFutures(), 0);
});

// Older bindings list the future family beside the async field.
test("a complete the table also lists still hands back a copy", async () => {
  const definitions = asyncDefinitions();
  definitions.functions[COMPLETE_RUST_BUFFER] = {
    args: [{ tag: "Handle" }],
    ret: { tag: "RustBuffer" },
    hasRustCallStatus: true,
  };
  const listed = register(definitions);
  const before = liveBuffers();
  const future = listed.uniffi_test_fn_future_buffer(16);
  await pollToReady(listed, future);
  const status = { code: 0 };
  const v = listed[COMPLETE_RUST_BUFFER](future, status);
  listed[FREE](future);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveBuffers(), before);
  listed.rustbuffer_free(v);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(v[15], 15);
  assert.strictEqual(liveFutures(), 0);
});

test("an empty RustBuffer result is an empty copy", async () => {
  const before = liveBuffers();
  const v = await completeBuffer(nm.uniffi_test_fn_future_buffer(0));
  assert.strictEqual(v.byteLength, 0);
  nm.rustbuffer_free(v);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
});

test("an async result passed back as an argument", async () => {
  const before = liveBuffers();
  const v = await completeBuffer(nm.uniffi_test_fn_future_buffer(16));
  const status = { code: 0 };
  if (process.versions.electron) {
    // Nothing is adopted under Copy, so the result is copied like any V8 array.
    assert.strictEqual(nm.uniffi_test_fn_buffer_len(v, status), 16);
  } else {
    // Its marker reads 0, as an adopted view's does, so Alias refuses it.
    assert.throws(
      () => nm.uniffi_test_fn_buffer_len(v, status),
      /or is an async result/,
    );
  }
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
});

test("the player's continuation shape wins over the table's", async () => {
  const definitions = asyncDefinitions();
  definitions.callbacks[CONTINUATION] = {
    args: [{ tag: "UInt32" }],
    ret: { tag: "Void" },
    hasRustCallStatus: false,
  };
  const other = register(definitions);
  const { value, status } = await completeU32(
    other,
    other.uniffi_test_fn_future_u32(7),
  );
  assert.strictEqual(value, 7);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveFutures(), 0);
});

test("a missing future export fails registration, naming it", () => {
  const definitions = asyncDefinitions();
  definitions.functions.uniffi_test_fn_future_u32.async.poll =
    "uniffi_test_no_such_poll";
  assert.throws(() => register(definitions), /uniffi_test_no_such_poll/);
  assert.strictEqual(liveFutures(), 0);
});

test("two async functions naming one complete export must agree on its return type", () => {
  const definitions = asyncDefinitions();
  const { uniffi_test_fn_future_u32: u32, uniffi_test_fn_future_error: error } =
    definitions.functions;
  assert.strictEqual(u32.async.complete, COMPLETE_U32);
  assert.strictEqual(error.async.complete, COMPLETE_U32);
  error.async.ret = { tag: "Int8" };
  assert.throws(() => register(definitions), new RegExp(COMPLETE_U32));
  assert.strictEqual(liveFutures(), 0);
});
