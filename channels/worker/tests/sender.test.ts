/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import {
  MessageChannel,
  type Transferable as NodeTransferable,
} from "node:worker_threads";
import {
  FfiType,
  type ModuleDefinitions,
  type UniffiRustCallStatus,
} from "@ubjs/core";
import { createSender, SenderCore } from "../src/sender.js";
import { ChannelClosedError } from "../src/types.js";
import { compilePlan } from "../src/plan.js";
import type { ChannelMessage } from "../src/messages.js";

const DEFS = {
  symbols: {
    rustbuffer_alloc: "a",
    rustbuffer_free: "f",
    rustbuffer_from_bytes: "b",
  },
  functions: {
    add: {
      args: [FfiType.UInt32, FfiType.UInt32],
      ret: FfiType.UInt32,
      hasRustCallStatus: true,
    },
    echo: {
      args: [FfiType.RustBuffer],
      ret: FfiType.RustBuffer,
      hasRustCallStatus: false,
    },
    take_cb: {
      args: [FfiType.Callback("Cb")],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    // Exercise makeForwarder: a callback whose own arg is itself a callback,
    // so the sender must forward that nested callback with "invocation" lifetime.
    take_with_inner: {
      args: [FfiType.Callback("WithInner")],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    take_with_nonvoid_inner: {
      args: [FfiType.Callback("WithNonVoidInner")],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
  },
  callbacks: {
    Cb: {
      args: [FfiType.Handle, FfiType.RustBuffer],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    Inner: {
      args: [FfiType.Int32],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    WithInner: {
      args: [FfiType.Callback("Inner")],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    NonVoidInner: { args: [], ret: FfiType.Int32, hasRustCallStatus: false },
    WithNonVoidInner: {
      args: [FfiType.Callback("NonVoidInner")],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
  },
  structs: {},
} satisfies ModuleDefinitions;

/** A hand-rolled peer standing in for the receiver. */
function peer(
  port: import("node:worker_threads").MessagePort,
  onMessage: (
    m: ChannelMessage,
    reply: (m: ChannelMessage, t?: NodeTransferable[]) => void,
  ) => void,
) {
  port.addEventListener("message", (ev) =>
    onMessage((ev as MessageEvent).data, (m, t) =>
      port.postMessage(m, t ?? []),
    ),
  );
}

test("a call posts args, and the return resolves the promise", async () => {
  const { port1, port2 } = new MessageChannel();
  const seen: ChannelMessage[] = [];
  peer(port2, (m, reply) => {
    seen.push(m);
    if (m.kind === "call")
      reply({
        kind: "return",
        id: m.id,
        ok: true,
        value: 3,
        status: { code: 0 },
      });
  });
  const s = createSender(DEFS, port1);
  const status: UniffiRustCallStatus = { code: 0 };
  const v = await s.add(1, 2, status);
  assert.strictEqual(v, 3);
  assert.deepStrictEqual(seen[0], {
    kind: "call",
    id: 1,
    fn: "add",
    args: [1, 2],
  }); // status stripped
  assert.strictEqual(status.code, 0);
  s.close();
  port2.close();
});

test("an error status is copied back and the value is undefined", async () => {
  const { port1, port2 } = new MessageChannel();
  peer(port2, (m, reply) => {
    if (m.kind === "call")
      reply({
        kind: "return",
        id: m.id,
        ok: true,
        value: 0,
        status: { code: 1, errorBuf: new Uint8Array([4, 2]) },
      });
  });
  const s = createSender(DEFS, port1);
  const status: UniffiRustCallStatus = { code: 0 };
  assert.strictEqual(await s.add(1, 2, status), undefined);
  assert.strictEqual(status.code, 1);
  assert.deepStrictEqual(Array.from(status.errorBuf!), [4, 2]);
  s.close();
  port2.close();
});

test("a receiver failure rejects with the rebuilt error", async () => {
  const { port1, port2 } = new MessageChannel();
  peer(port2, (m, reply) => {
    if (m.kind === "call")
      reply({
        kind: "return",
        id: m.id,
        ok: false,
        error: { name: "TypeError", message: "boom" },
      });
  });
  const s = createSender(DEFS, port1);
  await assert.rejects(
    () => s.echo(new Uint8Array(0)),
    (e: Error) => e.name === "TypeError" && e.message === "boom",
  );
  s.close();
  port2.close();
});

test("buffers are transferred: the caller's view is detached after the call", async () => {
  const { port1, port2 } = new MessageChannel();
  peer(port2, (m, reply) => {
    if (m.kind === "call") {
      const got = m.args[0] as Uint8Array;
      reply({ kind: "return", id: m.id, ok: true, value: got }, [
        got.buffer as ArrayBuffer,
      ]);
    }
  });
  const s = createSender(DEFS, port1);
  const view = s.rustbuffer_alloc(3);
  view.set([1, 2, 3]);
  const back = await s.echo(view);
  assert.strictEqual(view.byteLength, 0);
  assert.deepStrictEqual(Array.from(back), [1, 2, 3]);
  s.rustbuffer_free(view); // must not throw on a detached view
  s.close();
  port2.close();
});

test("a callback passed to a function is registered once and invoked by the peer", async () => {
  const { port1, port2 } = new MessageChannel();
  let cbId = -1;
  const replies: ChannelMessage[] = [];
  peer(port2, (m, reply) => {
    if (m.kind === "call") {
      cbId = m.args[0] as number;
      reply({ kind: "return", id: m.id, ok: true, value: undefined });
      // Fire the callback once, on the first registration only.
      if (m.id === 1)
        reply({
          kind: "callback",
          id: 77,
          cb: cbId,
          args: [9n, new Uint8Array([1])],
        });
    } else if (m.kind === "callback-return") {
      replies.push(m);
    }
  });
  const s = createSender(DEFS, port1);
  const calls: unknown[][] = [];
  const cb = (h: bigint, b: Uint8Array) => {
    calls.push([h, Array.from(b)]);
  };
  await s.take_cb(cb);
  await s.take_cb(cb);
  await new Promise((r) => setTimeout(r, 10));
  assert.strictEqual(cbId, 1);
  assert.deepStrictEqual(calls, [[9n, [1]]]);
  assert.deepStrictEqual(replies, [
    { kind: "callback-return", id: 77, ok: true, value: undefined },
  ]);
  s.close();
  port2.close();
});

test("a throwing callback becomes an ok:false callback-return", async () => {
  const { port1, port2 } = new MessageChannel();
  const replies: ChannelMessage[] = [];
  peer(port2, (m, reply) => {
    if (m.kind === "call") {
      reply({ kind: "return", id: m.id, ok: true, value: undefined });
      reply({
        kind: "callback",
        id: 5,
        cb: m.args[0] as number,
        args: [1n, new Uint8Array(0)],
      });
    } else if (m.kind === "callback-return") replies.push(m);
  });
  const s = createSender(DEFS, port1);
  await s.take_cb(() => {
    throw new RangeError("nope");
  });
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(replies, [
    {
      kind: "callback-return",
      id: 5,
      ok: false,
      error: { name: "RangeError", message: "nope" },
    },
  ]);
  s.close();
  port2.close();
});

test("close rejects outstanding calls and closes the port", async () => {
  const { port1, port2 } = new MessageChannel();
  peer(port2, () => {}); // never answers
  const s = createSender(DEFS, port1);
  const p = s.echo(new Uint8Array(0));
  s.close();
  await assert.rejects(p, ChannelClosedError);
  s.close(); // idempotent
  port2.close();
});

test("unknown function names fail synchronously", () => {
  const { port1, port2 } = new MessageChannel();
  const core = new SenderCore(compilePlan(DEFS), port1);
  assert.throws(() => core.invoke("nope", []), /unknown function "nope"/);
  core.close();
  port2.close();
});

test("an inbound release drops the callback from the registry", async () => {
  const { port1, port2 } = new MessageChannel();
  peer(port2, (m, reply) => {
    if (m.kind === "call")
      reply({ kind: "return", id: m.id, ok: true, value: undefined });
  });
  const core = new SenderCore(compilePlan(DEFS), port1);
  await core.invoke("take_cb", [() => {}]);
  assert.strictEqual(core.registrySize, 1);
  port2.postMessage({ kind: "release", cb: 1 });
  await new Promise((r) => setTimeout(r, 10));
  assert.strictEqual(core.registrySize, 0);
  core.close();
  port2.close();
});

test("a callback for an unknown id gets an ok:false callback-return", async () => {
  const { port1, port2 } = new MessageChannel();
  const replies: ChannelMessage[] = [];
  peer(port2, (m) => {
    if (m.kind === "callback-return") replies.push(m);
  });
  const core = new SenderCore(compilePlan(DEFS), port1);
  port2.postMessage({ kind: "callback", id: 9, cb: 42, args: [] });
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(replies, [
    {
      kind: "callback-return",
      id: 9,
      ok: false,
      error: { name: "Error", message: "unknown callback id 42" },
    },
  ]);
  core.close();
  port2.close();
});

test("makeForwarder forwards an invocation-lifetime callback, then releases it", async () => {
  const { port1, port2 } = new MessageChannel();
  const seen: ChannelMessage[] = [];
  let cbId = -1;
  peer(port2, (m, reply) => {
    seen.push(m);
    if (m.kind === "call") {
      cbId = m.args[0] as number;
      reply({ kind: "return", id: m.id, ok: true, value: undefined });
      // The peer stands in for Rust: it "owns" Inner callback id 99 and
      // hands it to our client through the WithInner callback's one arg.
      reply({ kind: "callback", id: 1, cb: cbId, args: [99] });
    }
  });
  const core = new SenderCore(compilePlan(DEFS), port1);
  const forwarderCounts: number[] = [];
  const client = (inner: (n: number) => void) => {
    forwarderCounts.push(core.forwarderCount); // built and cached: 1
    inner(7);
    forwarderCounts.push(core.forwarderCount); // released after the invocation-lifetime call: 0
  };
  await core.invoke("take_with_inner", [client]);
  await new Promise((r) => setTimeout(r, 10));
  assert.deepStrictEqual(forwarderCounts, [1, 0]);
  assert.strictEqual(core.forwarderCount, 0);
  const forwarded = seen.filter(
    (m) => m.kind === "callback" || m.kind === "release",
  );
  assert.strictEqual(forwarded.length, 2);
  const [cbMsg, relMsg] = forwarded;
  assert.strictEqual(cbMsg.kind, "callback");
  assert.strictEqual(
    (cbMsg as Extract<ChannelMessage, { kind: "callback" }>).cb,
    99,
  );
  assert.deepStrictEqual(
    (cbMsg as Extract<ChannelMessage, { kind: "callback" }>).args,
    [7],
  );
  assert.strictEqual(relMsg.kind, "release");
  assert.strictEqual(
    (relMsg as Extract<ChannelMessage, { kind: "release" }>).cb,
    99,
  );
  core.close();
  port2.close();
});

test("a forwarder for a non-Void callback throws instead of forwarding", async () => {
  const { port1, port2 } = new MessageChannel();
  let cbId = -1;
  peer(port2, (m, reply) => {
    if (m.kind === "call") {
      cbId = m.args[0] as number;
      reply({ kind: "return", id: m.id, ok: true, value: undefined });
      reply({ kind: "callback", id: 1, cb: cbId, args: [77] });
    }
  });
  const core = new SenderCore(compilePlan(DEFS), port1);
  let thrown: unknown;
  const client = (inner: () => number) => {
    try {
      inner();
    } catch (e) {
      thrown = e;
    }
  };
  await core.invoke("take_with_nonvoid_inner", [client]);
  await new Promise((r) => setTimeout(r, 10));
  assert.ok(thrown instanceof Error);
  assert.match(
    (thrown as Error).message,
    /callback "NonVoidInner" returning Int32 cannot be forwarded from the sender/,
  );
  core.close();
  port2.close();
});

test("the port is ref'd only while a call is outstanding", async () => {
  const { port1, port2 } = new MessageChannel();
  const events: string[] = [];
  // Wrap port1 so ref/unref calls are recorded; a real node MessagePort
  // already has both, but the test needs to observe when they're called.
  const port = {
    postMessage: (m: unknown, t?: readonly unknown[]) =>
      port1.postMessage(m, t as NodeTransferable[]),
    addEventListener: (type: "message", l: (ev: any) => void) =>
      port1.addEventListener(type, l),
    removeEventListener: (type: "message", l: (ev: any) => void) =>
      port1.removeEventListener(type, l),
    start: () => port1.start(),
    close: () => port1.close(),
    ref: () => events.push("ref"),
    unref: () => events.push("unref"),
  };
  const calls: Extract<ChannelMessage, { kind: "call" }>[] = [];
  peer(port2, (m) => {
    if (m.kind === "call") calls.push(m);
  });
  const s = createSender(DEFS, port);
  try {
    const p1 = s.add(1, 2, { code: 0 });
    const p2 = s.add(3, 4, { code: 0 });
    assert.deepStrictEqual(events, ["ref"]);
    await new Promise((r) => setTimeout(r, 10));
    assert.strictEqual(calls.length, 2);
    port2.postMessage({
      kind: "return",
      id: calls[0].id,
      ok: true,
      value: 3,
      status: { code: 0 },
    });
    await new Promise((r) => setTimeout(r, 10));
    assert.deepStrictEqual(events, ["ref"]);
    port2.postMessage({
      kind: "return",
      id: calls[1].id,
      ok: true,
      value: 7,
      status: { code: 0 },
    });
    await new Promise((r) => setTimeout(r, 10));
    assert.deepStrictEqual(events, ["ref", "unref"]);
    await p1;
    await p2;
  } finally {
    // A failed assertion above must not leave a real MessagePort open and
    // hanging the test process (unref bugs aside, ports here always close).
    s.close();
    port2.close();
  }
});

test("AsyncPlayer types the return and the trailing status", () => {
  const { port1, port2 } = new MessageChannel();
  const s = createSender(DEFS, port1);
  // Compile-time checks: these lines must type-check.
  const p1: Promise<number> = s.add(1, 2, { code: 0 });
  const p2: Promise<Uint8Array> = s.echo(new Uint8Array(0));
  const p3: Promise<void> = s.take_cb(() => {});
  // @ts-expect-error add takes a trailing status
  const bad: Promise<number> = s.add(1, 2);
  // No peer is listening; close() below rejects all four. Swallow so the
  // rejections don't surface as unhandled (this test only checks types).
  for (const p of [p1, p2, p3, bad]) p.catch(() => {});
  s.close();
  port2.close();
});
