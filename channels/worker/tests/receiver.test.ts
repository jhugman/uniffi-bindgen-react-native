/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import {
  MessageChannel,
  type MessagePort,
  type Transferable as NodeTransferable,
} from "node:worker_threads";
import { FfiType } from "@ubjs/core";
import { createReceiver, ReceiverCore } from "../src/receiver.js";
import { compilePlan } from "../src/plan.js";
import type { ChannelMessage } from "../src/messages.js";
import { DEFS, fakePlayer } from "./fixtures.js";

function senderSide(port: MessagePort) {
  const inbox: ChannelMessage[] = [];
  const waiters: ((m: ChannelMessage) => void)[] = [];
  port.addEventListener("message", (ev) => {
    const m = (ev as MessageEvent).data as ChannelMessage;
    const w = waiters.shift();
    if (w) w(m);
    else inbox.push(m);
  });
  const next = () =>
    new Promise<ChannelMessage>((r) => {
      const m = inbox.shift();
      if (m) r(m);
      else waiters.push(r);
    });
  const post = (m: ChannelMessage, t: NodeTransferable[] = []) =>
    port.postMessage(m, t);
  return { next, post };
}

test("call: status supplied, value returned, status copied", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const r = createReceiver(DEFS, player, port1);
  const s = senderSide(port2);
  s.post({ kind: "call", id: 1, fn: "add", args: [2, 3] });
  assert.deepStrictEqual(await s.next(), {
    kind: "return",
    id: 1,
    ok: true,
    value: 5,
    status: { code: 0 },
  });
  s.post({ kind: "call", id: 2, fn: "fail", args: [] });
  const m = (await s.next()) as any;
  assert.strictEqual(m.status.code, 1);
  assert.deepStrictEqual(Array.from(m.status.errorBuf), [7]);
  assert.strictEqual(m.value, undefined);
  r.close();
  port2.close();
});

test("call: unknown function and throwing player become ok:false", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  (player as any).boom = () => {
    throw new TypeError("kaboom");
  };
  const r = createReceiver(
    {
      ...DEFS,
      functions: {
        ...DEFS.functions,
        boom: { args: [], ret: FfiType.Void, hasRustCallStatus: false },
      },
    },
    player,
    port1,
  );
  const s = senderSide(port2);
  s.post({ kind: "call", id: 1, fn: "nope", args: [] });
  assert.deepStrictEqual(await s.next(), {
    kind: "return",
    id: 1,
    ok: false,
    error: {
      name: "Error",
      message: 'worker: unknown function "nope"',
    },
  });
  s.post({ kind: "call", id: 2, fn: "boom", args: [] });
  assert.deepStrictEqual(await s.next(), {
    kind: "return",
    id: 2,
    ok: false,
    error: { name: "TypeError", message: "kaboom" },
  });
  r.close();
  port2.close();
});

test("a returned RustBuffer is copied, the player's view freed, the copy transferred", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player, state } = fakePlayer();
  const r = createReceiver(DEFS, player, port1);
  const s = senderSide(port2);
  s.post({ kind: "call", id: 1, fn: "make_bytes", args: [] });
  const m = (await s.next()) as any;
  assert.deepStrictEqual(Array.from(m.value), [1, 2, 3]);
  assert.strictEqual(state.freed.length, 1);
  assert.notStrictEqual(state.freed[0], m.value);
  r.close();
  port2.close();
});

test("vtable init builds forwarders; a Void method posts a callback and returns at once", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const r = createReceiver(DEFS, player, port1);
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  assert.strictEqual((await s.next()).kind, "return");
  s.post({ kind: "call", id: 2, fn: "fire", args: [9n] });
  const [a, b] = [await s.next(), await s.next()];
  const cb = [a, b].find((m) => m.kind === "callback") as any;
  const ret = [a, b].find((m) => m.kind === "return");
  assert.ok(ret);
  assert.strictEqual(cb.cb, 3);
  assert.strictEqual(cb.args[0], 9n); // untagged on the way out (port 0)
  assert.deepStrictEqual(Array.from(cb.args[1]), [9]);
  r.close();
  port2.close();
});

test("clone/free are refcounted on the receiver; free is forwarded once at zero", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const core = new ReceiverCore(compilePlan(DEFS), player, port1, 0);
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  await s.next();
  s.post({ kind: "call", id: 2, fn: "clone_twice_free_thrice", args: [5n] });
  const msgs = [await s.next(), await s.next()];
  const cbs = msgs.filter((m) => m.kind === "callback") as any[];
  assert.strictEqual(cbs.length, 1);
  assert.strictEqual(cbs[0].cb, 1);
  assert.strictEqual(cbs[0].args[0], 5n);
  assert.strictEqual(core.refcounts.size, 0);
  core.close();
  port2.close();
});

test("vtable refcounts are keyed per interface: two interfaces sharing a handle number don't cross-decrement", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const core = new ReceiverCore(compilePlan(DEFS), player, port1, 0);
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  await s.next();
  s.post({
    kind: "call",
    id: 2,
    fn: "init_vtable2",
    args: [{ uniffi_free: 6, uniffi_clone: 7, notify: 8 }],
  });
  await s.next();
  // Same handle (5n) on both interfaces: clone on VT, free on VT2, free on VT twice.
  s.post({ kind: "call", id: 3, fn: "cross_vtable_refcount", args: [5n] });
  const msgs = [await s.next(), await s.next(), await s.next()];
  const cbs = msgs.filter((m) => m.kind === "callback") as any[];
  assert.strictEqual(cbs.length, 2);
  assert.ok(cbs.some((m) => m.cb === 6 && m.args[0] === 5n)); // VT2's free, forwarded at once (not refcounted against VT's clone)
  assert.ok(cbs.some((m) => m.cb === 1 && m.args[0] === 5n)); // VT's free, forwarded only once the clone is balanced
  assert.strictEqual(core.refcounts.size, 0);
  core.close();
  port2.close();
});

test("an async method returns a local dropped-struct immediately and maps the client's later", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player, state } = fakePlayer();
  const r = createReceiver(DEFS, player, port1);
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  await s.next();
  s.post({ kind: "call", id: 2, fn: "start_async", args: [5n] });
  const msgs = [await s.next(), await s.next()];
  const cb = msgs.find((m) => m.kind === "callback") as any;
  assert.strictEqual(cb.cb, 4);
  assert.strictEqual(typeof cb.args[1], "number"); // the completion callback crossed as an id
  assert.strictEqual(cb.args[2], 42n);
  assert.strictEqual(typeof state.dropped.handle, "bigint");
  assert.strictEqual(typeof state.dropped.free, "function");

  // Client replies with its real struct, whose `free` is client id 10.
  s.post({
    kind: "callback-return",
    id: cb.id,
    ok: true,
    value: { handle: 77n, free: 10 },
  });
  // Client fires the completion callback it was handed.
  s.post({ kind: "callback", id: 99, cb: cb.args[1], args: [42n, 123] });
  const cbRet = await s.next();
  assert.strictEqual(cbRet.kind, "callback-return");
  assert.deepStrictEqual(state.completions, [[42n, 123]]);

  // Rust drops the future: local free forwards to the client's free with the client's handle.
  s.post({ kind: "call", id: 3, fn: "drop_async", args: [] });
  const after = [await s.next(), await s.next(), await s.next()];
  const freeCb = after.find((m) => m.kind === "callback") as any;
  assert.strictEqual(freeCb.cb, 10);
  assert.strictEqual(freeCb.args[0], 77n);
  assert.ok(after.some((m) => m.kind === "release" && (m as any).cb === 10));
  r.close();
  port2.close();
});

test("a sync vtable method over an asynchronous port throws pointing at forceAsync", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player } = fakePlayer();
  const r = createReceiver(DEFS, player, port1);
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  await s.next();
  s.post({ kind: "call", id: 2, fn: "sync_method", args: [5n] });
  const msgs = [await s.next(), await s.next()];
  const ret = msgs.find((m) => m.kind === "return") as any;
  assert.strictEqual(ret.ok, false);
  assert.match(ret.error.message, /SyncM.*forceAsync/);
  r.close();
  port2.close();
});

test("handles are tagged with the port id on the way in and stripped on the way out", async () => {
  const { port1, port2 } = new MessageChannel();
  const { player, state } = fakePlayer();
  const r = createReceiver(DEFS, player, port1, { portId: 3 });
  const s = senderSide(port2);
  s.post({
    kind: "call",
    id: 1,
    fn: "init_vtable",
    args: [
      { uniffi_free: 1, uniffi_clone: 2, notify: 3, async_m: 4, sync_m: 5 },
    ],
  });
  await s.next();
  let seen: bigint | undefined;
  const realFire = player.fire;
  (player as any).fire = (h: bigint) => {
    seen = h;
    return realFire(h);
  };
  s.post({ kind: "call", id: 2, fn: "fire", args: [9n] });
  const msgs = [await s.next(), await s.next()];
  assert.strictEqual(seen, 9n | (3n << 48n));
  const cb = msgs.find((m) => m.kind === "callback") as any;
  assert.strictEqual(cb.args[0], 9n);
  void state;
  r.close();
  port2.close();
});
