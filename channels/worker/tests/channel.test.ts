/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { MessageChannel } from "node:worker_threads";
import { createSender } from "../src/sender.js";
import { createReceiver } from "../src/receiver.js";
import { DEFS, fakePlayer } from "./fixtures.js";

function pair() {
  const { port1, port2 } = new MessageChannel();
  const { player, state } = fakePlayer();
  const receiver = createReceiver(DEFS, player, port1);
  const sender = createSender(DEFS, port2);
  return {
    sender,
    receiver,
    player,
    state,
    done: () => {
      sender.close();
      receiver.close();
    },
  };
}

test("scalar call with status round-trips", async () => {
  const c = pair();
  const status = { code: 0 };
  assert.strictEqual(await c.sender.add(20, 22, status), 42);
  assert.strictEqual(status.code, 0);
  c.done();
});

test("the vtable is delivered and a Void callback reaches the client", async () => {
  const c = pair();
  const notified: unknown[] = [];
  await c.sender.init_vtable({
    uniffi_free: () => {},
    uniffi_clone: (h: bigint) => h,
    notify: (h: bigint, b: Uint8Array) => notified.push([h, Array.from(b)]),
    async_m: () => ({ handle: 0n, free: () => {} }),
    sync_m: () => ({ pointee: 1 }),
  });
  await c.sender.fire(9n);
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(notified, [[9n, [9]]]);
  c.done();
});

test("clone/free: the client's free runs exactly once", async () => {
  const c = pair();
  const freed: bigint[] = [];
  await c.sender.init_vtable({
    uniffi_free: (h: bigint) => freed.push(h),
    uniffi_clone: (h: bigint) => h,
    notify: () => {},
    async_m: () => ({ handle: 0n, free: () => {} }),
    sync_m: () => ({ pointee: 1 }),
  });
  await c.sender.clone_twice_free_thrice(5n);
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(freed, [5n]);
  c.done();
});

test("async method: completion and drop flow end to end", async () => {
  const c = pair();
  const clientFreed: bigint[] = [];
  await c.sender.init_vtable({
    uniffi_free: () => {},
    uniffi_clone: (h: bigint) => h,
    notify: () => {},
    sync_m: () => ({ pointee: 1 }),
    async_m: (
      _h: bigint,
      complete: (data: bigint, v: number) => void,
      data: bigint,
    ) => {
      setTimeout(() => complete(data, 7), 1);
      return { handle: 77n, free: (h: bigint) => clientFreed.push(h) };
    },
  });
  await c.sender.start_async(5n);
  await new Promise((r) => setTimeout(r, 20));
  assert.deepStrictEqual(c.state.completions, [[42n, 7]]);
  await c.sender.drop_async();
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(clientFreed, [77n]);
  c.done();
});

test("a client callback that throws surfaces as a receiver-side console error, not a hang", async () => {
  const c = pair();
  const errors: string[] = [];
  const orig = console.error;
  console.error = (...a: unknown[]) => errors.push(a.join(" "));
  try {
    await c.sender.init_vtable({
      uniffi_free: () => {},
      uniffi_clone: (h: bigint) => h,
      notify: () => {
        throw new Error("client failed");
      },
      async_m: () => ({ handle: 0n, free: () => {} }),
      sync_m: () => ({ pointee: 1 }),
    });
    await c.sender.fire(1n);
    await new Promise((r) => setTimeout(r, 10));
  } finally {
    console.error = orig;
  }
  assert.ok(errors.some((e) => e.includes("client failed")));
  c.done();
});

test("two ports: each callback reaches the sender that registered it, untagged", async () => {
  // v2 shape on v1 code: two receivers, two port ids, one fake player whose
  // routing stands in for the receiver-side attach() to come.
  const { player, state } = fakePlayer();
  const vtables: any[] = [];
  (player as any).init_vtable = (vt: any) => {
    vtables.push(vt);
  };
  (player as any).fire = (h: bigint) =>
    vtables[Number(h >> 48n)].notify(h, new Uint8Array(0));
  const a = new MessageChannel();
  const b = new MessageChannel();
  const ra = createReceiver(DEFS, player, a.port1, { portId: 0 });
  const rb = createReceiver(DEFS, player, b.port1, { portId: 1 });
  const sa = createSender(DEFS, a.port2);
  const sb = createSender(DEFS, b.port2);
  const gotA: bigint[] = [],
    gotB: bigint[] = [];
  const vt = (sink: bigint[]) => ({
    uniffi_free: () => {},
    uniffi_clone: (h: bigint) => h,
    notify: (h: bigint) => sink.push(h),
    async_m: () => ({ handle: 0n, free: () => {} }),
    sync_m: () => ({ pointee: 1 }),
  });
  await sa.init_vtable(vt(gotA));
  await sb.init_vtable(vt(gotB));
  await sa.fire(1n);
  await sb.fire(1n);
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(gotA, [1n]);
  assert.deepStrictEqual(gotB, [1n]);
  void state;
  sa.close();
  sb.close();
  ra.close();
  rb.close();
});
