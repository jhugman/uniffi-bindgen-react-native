/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import { BRIDGE_KEY, CONNECT, DISCONNECTED, PORT } from "../src/protocol.js";
import { installPreload } from "../src/preload/core.js";
import { fakeRenderer, stubPort } from "./fakes/preload.js";
import { ID } from "./fakes/helpers.js";

const OTHER = "fedcba9876543210fedcba9876543210";

function installed() {
  const r = fakeRenderer();
  installPreload(r.deps);
  return { ...r, bridge: r.exposed.get(BRIDGE_KEY) };
}

function shippedPreload(): string {
  const url = new URL("../dist/preload.cjs", import.meta.url);
  assert.ok(
    existsSync(url),
    "dist/preload.cjs is missing: run npm run build first",
  );
  return readFileSync(url, "utf8");
}

test("refuses to load without context isolation", () => {
  const r = fakeRenderer(false);
  assert.throws(() => installPreload(r.deps), /contextIsolation/);
  assert.strictEqual(r.exposed.size, 0);
  assert.strictEqual(r.listenerCount(), 0);
});

test("exposes only protocol and connect", () => {
  const { exposed, bridge } = installed();
  assert.deepStrictEqual([...exposed.keys()], [BRIDGE_KEY]);
  assert.deepStrictEqual(Object.keys(bridge).sort(), ["connect", "protocol"]);
  assert.strictEqual(bridge.protocol, 1);
});

test("connect sends the id, and a malformed id throws, sends nothing and keeps the pending id", () => {
  const { bridge, sent, deliver, posted } = installed();
  bridge.connect(ID);
  assert.deepStrictEqual(sent, [[CONNECT, { id: ID }]]);
  // Matched by message: across contextBridge the page sees a plain Error.
  assert.throws(() => bridge.connect("x"), /32 lowercase hex/);
  assert.strictEqual(sent.length, 1);
  const p = stubPort();
  deliver(PORT, { id: ID, info: null }, [p]);
  assert.deepStrictEqual(posted[0]?.ports, [p]);
});

test("a reply for any other id is dropped and its ports closed", () => {
  const { bridge, deliver, posted } = installed();
  const early = stubPort();
  deliver(PORT, { id: ID, info: null }, [early]);
  assert.strictEqual(early.closed, true);
  const noId = stubPort();
  deliver(PORT, { info: null }, [noId]);
  assert.strictEqual(noId.closed, true);

  bridge.connect(ID);
  const other = stubPort();
  deliver(PORT, { id: OTHER, info: null }, [other]);
  assert.strictEqual(other.closed, true);

  const malformed = stubPort();
  deliver(PORT, null, [malformed]);
  assert.strictEqual(malformed.closed, true);
  assert.deepStrictEqual(posted, []);
});

test("a later connect replaces the pending id", () => {
  const { bridge, deliver, posted } = installed();
  bridge.connect(ID);
  bridge.connect(OTHER);
  const p = stubPort();
  deliver(PORT, { id: ID, info: null }, [p]);
  assert.deepStrictEqual(posted, []);
  assert.strictEqual(p.closed, true);
});

test("a disconnected notice reaches the page only for the pending id, and its ports are closed", () => {
  const { bridge, deliver, posted } = installed();
  bridge.connect(ID);
  const stray = stubPort();
  deliver(DISCONNECTED, { id: OTHER, code: 9, reason: "utility-exited" }, [
    stray,
  ]);
  assert.deepStrictEqual(posted, []);
  assert.strictEqual(stray.closed, true);
  deliver(DISCONNECTED, { id: ID, code: 9, reason: "utility-exited" });
  assert.deepStrictEqual(posted, [
    {
      message: {
        type: DISCONNECTED,
        id: ID,
        code: 9,
        reason: "utility-exited",
      },
      origin: "*",
      ports: [],
    },
  ]);
});

test("dist/preload.cjs requires only electron and has no module syntax", () => {
  const source = shippedPreload();
  const requires = [...source.matchAll(/\brequire\(\s*(["'])(.*?)\1\s*\)/g)];
  assert.ok(requires.length > 0, "the preload never requires electron");
  for (const [call, , name] of requires)
    assert.strictEqual(name, "electron", call);
  assert.strictEqual(source.split("require(").length - 1, requires.length);
  assert.doesNotMatch(source, /^\s*(import|export)\b/m);
});

/** Runs the shipped bundle as a preload would, with only electron to require. */
function runShipped(contextIsolated: boolean) {
  const r = fakeRenderer();
  const { ipcRenderer, contextBridge, window } = r.deps;
  const require = (name: string) => {
    if (name === "electron") return { ipcRenderer, contextBridge };
    throw new Error(`the preload required ${name}`);
  };
  vm.runInNewContext(shippedPreload(), {
    require,
    process: { contextIsolated },
    window,
    module: { exports: {} },
    exports: {},
  });
  return r;
}

test("the pending id outlives its port, so a later death still reaches the page", () => {
  const { bridge, deliver, posted } = installed();
  const p = stubPort();
  bridge.connect(ID);
  deliver(PORT, { id: ID, info: null }, [p]);
  deliver(DISCONNECTED, { id: ID, code: 9, reason: "utility-exited" });
  assert.deepStrictEqual(
    posted.map((m) => (m.message as { type: string }).type),
    [PORT, DISCONNECTED],
  );
  assert.deepStrictEqual(posted[0].ports, [p]);
});

test("a second copy of the preload throws before it listens, so it installs nothing", () => {
  const r = fakeRenderer();
  installPreload(r.deps);
  const listeners = r.listenerCount();
  assert.throws(() => installPreload(r.deps), /existing property/);
  assert.strictEqual(r.listenerCount(), listeners);
  const p = stubPort();
  r.exposed.get(BRIDGE_KEY).connect(ID);
  r.deliver(PORT, { id: ID, info: null }, [p]);
  assert.strictEqual(p.closed, false);
  assert.strictEqual(r.posted.length, 1);
});
