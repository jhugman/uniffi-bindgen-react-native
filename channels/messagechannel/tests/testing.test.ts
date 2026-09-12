/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { createSyncPortPair, createSyncPlayer } from "../src/testing.js";
import { createReceiver } from "../src/receiver.js";
import { DEFS, fakePlayer } from "./fixtures.js";

test("sync port delivers synchronously and queues until a listener attaches", () => {
  const [a, b] = createSyncPortPair();
  a.postMessage({ kind: "release", cb: 1 });
  const got: unknown[] = [];
  b.addEventListener("message", (ev) => got.push(ev.data));
  assert.deepStrictEqual(got, [{ kind: "release", cb: 1 }]);
  a.postMessage({ kind: "release", cb: 2 });
  assert.deepStrictEqual(got.length, 2);
});

test("a sync player returns values synchronously through a real receiver", () => {
  const [rx, tx] = createSyncPortPair();
  const { player } = fakePlayer();
  const receiver = createReceiver(DEFS, player, rx);
  const sync = createSyncPlayer(DEFS, tx);
  const status = { code: 0 };
  assert.strictEqual(sync.add(1, 2, status), 3);
  const bytes = sync.make_bytes();
  assert.deepStrictEqual(Array.from(bytes), [1, 2, 3]);
  sync.close();
  receiver.close();
});

test("a sync vtable method works over the sync port", () => {
  const [rx, tx] = createSyncPortPair();
  const { player } = fakePlayer();
  const receiver = createReceiver(DEFS, player, rx);
  const sync = createSyncPlayer(DEFS, tx);
  sync.init_vtable({
    uniffi_free: () => {},
    uniffi_clone: (h: bigint) => h,
    notify: () => {},
    async_m: () => ({ handle: 0n, free: () => {} }),
    sync_m: (h: bigint) => ({ pointee: Number(h) + 1 }),
  });
  assert.deepStrictEqual(sync.sync_method(4n), { pointee: 5 });
  sync.close();
  receiver.close();
});

test("invokeSync over an asynchronous port throws instead of hanging", async () => {
  const { MessageChannel } = await import("node:worker_threads");
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const receiver = createReceiver(DEFS, player, port1);
  const sync = createSyncPlayer(DEFS, port2);
  assert.throws(
    () => sync.add(1, 2, { code: 0 }),
    /did not return synchronously/,
  );
  sync.close();
  receiver.close();
});
