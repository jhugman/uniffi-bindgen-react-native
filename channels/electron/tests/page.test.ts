/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { once } from "node:events";
import { test, type TestContext } from "node:test";
import { createSender } from "@ubjs/worker";
import { createPage, type PageWindow } from "../src/page/core.js";
import { DISCONNECTED, PORT } from "../src/protocol.js";
import { fakeWindow } from "./fakes/page.js";
import { DEFS } from "./fakes/player.js";
import { stubPort } from "./fakes/preload.js";
import { within } from "./fakes/helpers.js";

const OTHER = "fedcba9876543210fedcba9876543210";

/** Whether `promise` has settled, once pending callbacks have run. */
async function isSettled(promise: Promise<unknown>): Promise<boolean> {
  let settled = false;
  promise.then(
    () => (settled = true),
    () => (settled = true),
  );
  await new Promise((r) => setImmediate(r));
  return settled;
}

/** A connected page, over port1 of a real channel that the test closes after. */
async function connected(t: TestContext) {
  const fx = fakeWindow();
  const page = createPage(fx.win);
  const pending = page.connect();
  const [id] = fx.ids;
  const { port1, port2 } = new MessageChannel();
  t.after(() => port2.close());
  fx.dispatch({ type: PORT, id, info: { dataDir: "/d" } }, { ports: [port1] });
  return { fx, id, port2, conn: await pending };
}

test("isAvailable needs a bridge with protocol 1", () => {
  assert.strictEqual(createPage(fakeWindow("none").win).isAvailable(), false);
  assert.strictEqual(createPage(fakeWindow(2).win).isAvailable(), false);
  assert.strictEqual(createPage(fakeWindow(1).win).isAvailable(), true);
});

test("without a bridge, connect rejects with unavailable", async () => {
  for (const bridge of ["none", 2] as const) {
    const fx = fakeWindow(bridge);
    await assert.rejects(createPage(fx.win).connect(), {
      name: "ConnectError",
      reason: "unavailable",
    });
    assert.strictEqual(fx.listenerCount(), 0, `bridge ${bridge}`);
  }
});

test("a bridge that throws rejects with unavailable, and connect does not throw", async () => {
  const win: PageWindow = {
    addEventListener() {},
    removeEventListener() {},
    __ubjsElectron: {
      protocol: 1,
      connect() {
        throw new TypeError("cannot send");
      },
    },
  };
  await assert.rejects(createPage(win).connect(), {
    name: "ConnectError",
    reason: "unavailable",
    message: /cannot send/,
  });
});

test("connect listens before it asks, with a fresh 32-hex id", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const a = fakeWindow();
  const b = fakeWindow();
  createPage(a.win).connect();
  createPage(b.win).connect();
  assert.strictEqual(a.ids.length, 1);
  assert.match(a.ids[0], /^[0-9a-f]{32}$/);
  assert.deepStrictEqual(a.listenersAtAsk, [1], "listens before it asks");
  assert.strictEqual(a.listenerCount(), 1);
  assert.notStrictEqual(a.ids[0], b.ids[0]);
});

test("an error reply rejects with its reason and message", async () => {
  for (const reason of ["not-served", "start-failed", "timeout"] as const) {
    const fx = fakeWindow();
    const pending = createPage(fx.win).connect();
    fx.dispatch({
      type: PORT,
      id: fx.ids[0],
      error: { reason, message: `because ${reason}` },
    });
    await assert.rejects(pending, {
      name: "ConnectError",
      reason,
      message: `because ${reason}`,
    });
  }
});

test("messages from another source, of another type, or for another id are ignored, and ports on our type are closed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  const pending = createPage(fx.win).connect();
  const [id] = fx.ids;

  const elsewhere = stubPort();
  fx.dispatch(
    { type: PORT, id, info: null },
    { source: {}, ports: [elsewhere] },
  );
  assert.ok(elsewhere.closed, "another source");
  assert.strictEqual(await isSettled(pending), false);

  const theApps = stubPort();
  fx.dispatch({ type: "app:port", id, info: null }, { ports: [theApps] });
  fx.dispatch(null);
  fx.dispatch("ubjs-electron:v1:port");
  assert.strictEqual(
    theApps.closed,
    false,
    "another type is not ours to close",
  );
  assert.strictEqual(await isSettled(pending), false);

  const reply = stubPort();
  const notice = stubPort();
  fx.dispatch({ type: PORT, id: OTHER, info: null }, { ports: [reply] });
  fx.dispatch(
    { type: DISCONNECTED, id: OTHER, code: 9, reason: "utility-exited" },
    { ports: [notice] },
  );
  assert.ok(reply.closed && notice.closed, "another id");
  assert.strictEqual(await isSettled(pending), false);
});

test("a port message with no port or two ports is refused and its ports closed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  const pending = createPage(fx.win).connect();
  const [id] = fx.ids;

  fx.dispatch({ type: PORT, id, info: null });
  assert.strictEqual(await isSettled(pending), false);

  const [a, b] = [stubPort(), stubPort()];
  fx.dispatch({ type: PORT, id, info: null }, { ports: [a, b] });
  assert.ok(a.closed && b.closed);
  assert.strictEqual(await isSettled(pending), false);
});

test("no reply within timeoutMs rejects with timeout naming /main, and a late port is closed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  const pending = createPage(fx.win).connect({ timeoutMs: 50 });
  const [id] = fx.ids;

  t.mock.timers.tick(49);
  assert.strictEqual(await isSettled(pending), false);
  t.mock.timers.tick(1);
  await assert.rejects(pending, {
    name: "ConnectError",
    reason: "timeout",
    message: /@ubjs\/electron\/main/,
  });

  const late = stubPort();
  fx.dispatch({ type: PORT, id, info: null }, { ports: [late] });
  assert.ok(late.closed);
});

test("a malformed error reply for our id still closes its ports", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  createPage(fx.win).connect();
  const p = stubPort();
  assert.throws(() =>
    fx.dispatch({ type: PORT, id: fx.ids[0], error: null }, { ports: [p] }),
  );
  assert.ok(p.closed);
});

test("connect is memoized", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  const page = createPage(fx.win);
  const first = page.connect();
  assert.strictEqual(page.connect({ timeoutMs: 5 }), first);
  assert.strictEqual(fx.ids.length, 1);
  assert.strictEqual(fx.listenerCount(), 1);
});

test("closing the port settles disconnected as closed, and a later notice changes nothing", async (t) => {
  const { fx, id, port2, conn } = await connected(t);
  const peerHeard = once(port2, "close");

  conn.port.close();
  assert.deepStrictEqual(await conn.disconnected, {
    code: null,
    reason: "closed",
  });
  await within(peerHeard);

  fx.dispatch({ type: DISCONNECTED, id, code: 9, reason: "utility-exited" });
  assert.deepStrictEqual(await conn.disconnected, {
    code: null,
    reason: "closed",
  });
});

test("a disconnected notice while waiting rejects with start-failed, and a later port is closed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const fx = fakeWindow();
  const pending = createPage(fx.win).connect();
  const [id] = fx.ids;

  fx.dispatch({ type: DISCONNECTED, id, code: 1, reason: "utility-exited" });
  await assert.rejects(pending, {
    name: "ConnectError",
    reason: "start-failed",
    message: "the utility exited before the page got its port",
  });

  const late = stubPort();
  fx.dispatch({ type: PORT, id, info: null }, { ports: [late] });
  assert.ok(late.closed);
});
