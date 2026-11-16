/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import type { UniffiRustCallStatus } from "@ubjs/core";
import { createSender } from "../src/sender.js";
import { createReceiver } from "../src/receiver.js";
import type { ChannelPort } from "../src/port.js";
import { DEFS, fakePlayer } from "./fixtures.js";

const tick = () => new Promise((r) => setImmediate(r));
const status = (): UniffiRustCallStatus => ({ code: 0 });

// Nothing here holds the event loop open, so a call that never settles would
// drain the loop and cancel every later test; this timer keeps each failure its own.
function within<T>(promise: Promise<T>, ms = 1000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`not settled within ${ms} ms`)),
      ms,
    );
  });
  return Promise.race([promise, late]).finally(() => clearTimeout(timer));
}

function closablePort(closed?: PromiseLike<unknown>) {
  let settle!: () => void, fail!: (e: unknown) => void;
  const own = new Promise<void>((res, rej) => {
    settle = res;
    fail = rej;
  });
  const posted: any[] = [];
  let listener: ((ev: { data: unknown }) => void) | undefined;
  let closes = 0;
  // Ignores addEventListener's type, as receiver.test.ts's adapter does.
  const port: ChannelPort = {
    postMessage: (m) => void posted.push(m),
    addEventListener: (_t, l) => void (listener = l),
    removeEventListener: () => void (listener = undefined),
    close: () => void closes++,
    closed: closed ?? own,
  };
  return {
    port,
    posted,
    settle,
    fail,
    deliver: (data: unknown) => listener?.({ data }),
    closes: () => closes,
  };
}

const callId = (posted: any[], fn: string): number =>
  posted.find((m) => m.kind === "call" && m.fn === fn).id;
const cancels = (posted: any[]) => posted.filter((m) => m.kind === "cancel");
const closedError = { name: "ChannelClosedError" };

test("a settled closed rejects every call in flight and posts one cancel per async call", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  const calls = [
    assert.rejects(within(sender.add(1, 2, status())), closedError),
    assert.rejects(within(sender.sleep(5, status(), undefined)), closedError),
  ];
  p.settle();
  await Promise.all(calls);
  assert.deepStrictEqual(cancels(p.posted), [
    { kind: "cancel", id: callId(p.posted, "sleep") },
  ]);
});

test("a call after closed settles rejects at once and posts nothing", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  p.settle();
  await tick();
  await assert.rejects(within(sender.add(1, 2, status())), closedError);
  assert.deepStrictEqual(p.posted, []);
});

test("a closed that settled before the sender existed closes it", async () => {
  const p = closablePort(Promise.resolve());
  const sender = createSender(DEFS, p.port);
  await tick();
  await assert.rejects(within(sender.add(1, 2, status())), closedError);
  assert.deepStrictEqual(p.posted, []);
});

test("a rejected closed closes the sender too", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  const add = assert.rejects(within(sender.add(1, 2, status())), closedError);
  p.fail(new Error("gone"));
  await add;
});

test("closed and an explicit close() close the port once", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  const sleep = assert.rejects(
    within(sender.sleep(5, status(), undefined)),
    closedError,
  );
  p.settle();
  sender.close();
  await tick();
  await sleep;
  assert.strictEqual(p.closes(), 1);
  assert.deepStrictEqual(cancels(p.posted), [
    { kind: "cancel", id: callId(p.posted, "sleep") },
  ]);
});

test("a type-ignoring adapter does not mistake a message for closed", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  const add = sender.add(1, 2, status());
  const id = callId(p.posted, "add");
  p.deliver({ kind: "return", id, ok: true, value: 3, status: { code: 0 } });
  assert.strictEqual(await within(add), 3);
  assert.strictEqual(p.closes(), 0);
});

test("a port without closed behaves as before", async () => {
  const p = closablePort();
  delete p.port.closed;
  const sender = createSender(DEFS, p.port);
  const add = sender.add(1, 2, status());
  const id = callId(p.posted, "add");
  p.deliver({ kind: "return", id, ok: true, value: 3, status: { code: 0 } });
  assert.strictEqual(await within(add), 3);
});

test("a receiver aborts its in-flight entries when closed settles, and posts nothing after", async () => {
  const p = closablePort();
  const { player, state } = fakePlayer();
  createReceiver(DEFS, player, p.port);
  p.deliver({ kind: "call", id: 1, fn: "sleep", args: [10_000] });
  p.settle();
  await tick();
  assert.strictEqual(state.cancelled, 1);
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(p.posted, []);
});

test("a port that refuses posts once closed still rejects every call in flight", async () => {
  const p = closablePort();
  const sender = createSender(DEFS, p.port);
  const calls = [
    assert.rejects(within(sender.sleep(5, status(), undefined)), closedError),
    assert.rejects(within(sender.sleep(6, status(), undefined)), closedError),
  ];
  p.port.postMessage = () => {
    throw new Error("the far end is gone");
  };
  p.settle();
  await Promise.all(calls);
  assert.strictEqual(p.closes(), 1);
});

test("a throw from closing on closed is warned, not left unhandled", async (t) => {
  const warn = t.mock.method(console, "warn", () => {});
  const p = closablePort();
  p.port.close = () => {
    throw new Error("refused");
  };
  createSender(DEFS, p.port);
  p.settle();
  await tick();
  assert.strictEqual(warn.mock.callCount(), 1);
  assert.match(String(warn.mock.calls[0].arguments[0]), /port closed/);
});

test("a closed that is not a promise is ignored", async () => {
  const notAPromise = false as unknown as PromiseLike<unknown>;
  const p = closablePort(notAPromise);
  const sender = createSender(DEFS, p.port);
  const add = sender.add(1, 2, status());
  const id = callId(p.posted, "add");
  p.deliver({ kind: "return", id, ok: true, value: 3, status: { code: 0 } });
  assert.strictEqual(await within(add), 3);
  const { player } = fakePlayer();
  assert.doesNotThrow(() =>
    createReceiver(DEFS, player, closablePort(notAPromise).port),
  );
});
