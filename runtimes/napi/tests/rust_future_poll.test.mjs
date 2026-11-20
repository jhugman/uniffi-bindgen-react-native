/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// test_lib's futures, driven through future exports the table names itself: the path
// bindings take when they run the poll loop in JS.
import { test } from "node:test";
import assert from "node:assert";
import lib from "../lib.js";
const { UniffiNativeModule } = lib;
import { libPath } from "./helpers/lib-path.mjs";
import { legacyDefinitions, pollToReady } from "./helpers/test-lib-futures.mjs";

const nm = UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
  legacyDefinitions(),
);

const liveFutures = () => nm.uniffi_test_live_future_count({ code: 0 });
const liveBuffers = () => nm.uniffi_test_live_buffer_count({ code: 0 });

test("a ready future completes with its value", async () => {
  const f = nm.uniffi_test_fn_future_u32(7);
  await pollToReady(nm, f);
  const status = { code: 0 };
  assert.strictEqual(nm.uniffi_test_rust_future_complete_u32(f, status), 7);
  assert.strictEqual(status.code, 0);
  nm.uniffi_test_rust_future_free(f);
  assert.strictEqual(liveFutures(), 0);
});

test("a future woken from another thread completes", async () => {
  const f = nm.uniffi_test_fn_future_from_thread(9, 5);
  await pollToReady(nm, f);
  const status = { code: 0 };
  assert.strictEqual(nm.uniffi_test_rust_future_complete_u32(f, status), 9);
  assert.strictEqual(status.code, 0);
  nm.uniffi_test_rust_future_free(f);
  assert.strictEqual(liveFutures(), 0);
});

test("a RustBuffer future's bytes come back, and rustbuffer_free releases them", async () => {
  const before = liveBuffers();
  const f = nm.uniffi_test_fn_future_buffer(16);
  await pollToReady(nm, f);
  const status = { code: 0 };
  const view = nm.uniffi_test_rust_future_complete_rust_buffer(f, status);
  assert.strictEqual(status.code, 0);
  // Alias hands back the library's allocation; Copy has already freed it.
  assert.strictEqual(
    liveBuffers(),
    before + (process.versions.electron ? 0 : 1),
  );
  nm.uniffi_test_rust_future_free(f);
  assert.strictEqual(view.byteLength, 16);
  assert.strictEqual(view[15], 15);
  nm.rustbuffer_free(view);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
});

test("an erroring future completes with CALL_ERROR and its error buffer", async () => {
  const before = liveBuffers();
  const f = nm.uniffi_test_fn_future_error();
  await pollToReady(nm, f);
  const status = { code: 0 };
  nm.uniffi_test_rust_future_complete_u32(f, status);
  nm.uniffi_test_rust_future_free(f);
  assert.strictEqual(status.code, 1);
  assert.deepStrictEqual([...status.errorBuf], [1, 2, 3]);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
});

test("a parked future cancelled mid-flight completes as cancelled", async () => {
  const f = nm.uniffi_test_fn_future_parked();
  const ready = pollToReady(nm, f);
  nm.uniffi_test_rust_future_cancel(f);
  await ready;
  const status = { code: 0 };
  nm.uniffi_test_rust_future_complete_u32(f, status);
  nm.uniffi_test_rust_future_free(f);
  assert.strictEqual(status.code, 3);
  assert.strictEqual(liveFutures(), 0);
});
