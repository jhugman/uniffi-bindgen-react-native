/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The async entries lib.js composes: `(...args, status, signal) => Promise` per async function.
//
// This file imports @ubjs/core's ESM build, and lib.js requires its CJS build, so the resolver
// counts below read a map the two copies share.
import { test } from "node:test";
import assert from "node:assert";
import { createRequire } from "node:module";
import { uniffiRustFutureHandleCount } from "@ubjs/core";
import lib from "../lib.js";
const { UniffiNativeModule } = lib;
import { libPath } from "./helpers/lib-path.mjs";
import { asyncDefinitions } from "./helpers/test-lib-futures.mjs";

const LIB = libPath("uniffi_napi_test_lib");

const register = () =>
  UniffiNativeModule.open(LIB).register(asyncDefinitions());

const nm = register();
const liveFutures = () => nm.uniffi_test_live_future_count({ code: 0 });
const liveBuffers = () => nm.uniffi_test_live_buffer_count({ code: 0 });

test("an async entry resolves with the completed value", async () => {
  const status = { code: 0 };
  assert.strictEqual(
    await nm.uniffi_test_fn_future_u32(7, status, undefined),
    7,
  );
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveFutures(), 0);
});

test("an entry has started its future before it returns, and an abort cancels it", async () => {
  const futuresBefore = liveFutures();
  const resolversBefore = uniffiRustFutureHandleCount();
  const status = { code: 0 };
  const c = new AbortController();
  const p = nm.uniffi_test_fn_future_parked(status, c.signal);
  // Read now, assert after the abort: a failed assertion must not leave the future parked.
  const started = liveFutures();
  // The entry's first poll inserted a resolver through lib.js's copy of @ubjs/core.
  const resolvers = uniffiRustFutureHandleCount();
  c.abort();
  await p;
  assert.strictEqual(started, futuresBefore + 1);
  assert.strictEqual(resolvers, resolversBefore + 1);
  assert.strictEqual(status.code, 3);
  assert.strictEqual(liveFutures(), futuresBefore);
  assert.strictEqual(uniffiRustFutureHandleCount(), resolversBefore);
  assert.strictEqual(liveFutures(), 0);
});

test("an entry woken from another thread resolves", async () => {
  const status = { code: 0 };
  assert.strictEqual(
    await nm.uniffi_test_fn_future_from_thread(9, 5, status, undefined),
    9,
  );
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveFutures(), 0);
});

test("a RustBuffer result is a JS-owned copy", async () => {
  const before = liveBuffers();
  const status = { code: 0 };
  const v = await nm.uniffi_test_fn_future_buffer(64, status, undefined);
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveBuffers(), before);
  nm.rustbuffer_free(v);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(v.byteLength, 64);
  assert.strictEqual(v[63], 63);
  assert.strictEqual(liveFutures(), 0);
});

test("an errored future resolves with CALL_ERROR and frees its error buffer", async () => {
  const before = liveBuffers();
  const status = { code: 0 };
  await nm.uniffi_test_fn_future_error(status, undefined);
  assert.strictEqual(status.code, 1);
  assert.deepStrictEqual([...status.errorBuf], [1, 2, 3]);
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
});

test("many awaits leave nothing allocated", async () => {
  await nm.uniffi_test_fn_future_u32(0, { code: 0 }, undefined);
  const t0 = nm.$uniffiTrampolineCount();
  const before = liveBuffers();
  for (let i = 0; i < 100; i++) {
    const status = { code: 0 };
    assert.strictEqual(
      await nm.uniffi_test_fn_future_u32(i, status, undefined),
      i,
    );
    const v = await nm.uniffi_test_fn_future_buffer(64, status, undefined);
    assert.strictEqual(v[63], 63);
    await nm.uniffi_test_fn_future_error(status, undefined);
    assert.strictEqual(status.code, 1);
    status.code = 0;
    assert.strictEqual(
      await nm.uniffi_test_fn_future_from_thread(i, 0, status, undefined),
      i,
    );
    const c = new AbortController();
    const parked = nm.uniffi_test_fn_future_parked(status, c.signal);
    c.abort();
    await parked;
    assert.strictEqual(status.code, 3);
  }
  assert.strictEqual(liveBuffers(), before);
  assert.strictEqual(liveFutures(), 0);
  assert.strictEqual(nm.$uniffiTrampolineCount(), t0);
  assert.strictEqual(uniffiRustFutureHandleCount(), 0);
});

test("futures woken out of order each resolve with their own value", async () => {
  const indices = Array.from({ length: 32 }, (_, i) => i);
  const values = await Promise.all(
    indices.map((i) =>
      nm.uniffi_test_fn_future_from_thread(
        i,
        (31 - i) % 7,
        { code: 0 },
        undefined,
      ),
    ),
  );
  assert.deepStrictEqual(values, indices);
  assert.strictEqual(liveFutures(), 0);
});

test("two registrations of one library each drive their own futures", async () => {
  const other = register();
  const values = await Promise.all([
    nm.uniffi_test_fn_future_from_thread(1, 3, { code: 0 }, undefined),
    other.uniffi_test_fn_future_from_thread(2, 1, { code: 0 }, undefined),
  ]);
  assert.deepStrictEqual(values, [1, 2]);
  assert.strictEqual(liveFutures(), 0);
});

test("lib.js evaluated again wraps the native register only once", async () => {
  // Jest gives each test file its own module registry but shares the cached addon, so
  // lib.js runs again against the same class.
  const require = createRequire(import.meta.url);
  delete require.cache[require.resolve("../lib.js")];
  const again = require("../lib.js");
  assert.strictEqual(again.UniffiNativeModule, UniffiNativeModule);

  const fresh = again.UniffiNativeModule.open(LIB).register(asyncDefinitions());
  const status = { code: 0 };
  assert.strictEqual(
    await fresh.uniffi_test_fn_future_u32(5, status, undefined),
    5,
  );
  assert.strictEqual(status.code, 0);
  assert.strictEqual(liveFutures(), 0);
});
