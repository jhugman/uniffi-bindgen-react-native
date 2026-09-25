/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { MessageChannel } from "node:worker_threads";
import { type ChannelPort } from "../src/port.js";
import {
  toWireError,
  fromWireError,
  isChannelMessage,
} from "../src/messages.js";

test("Node's MessagePort satisfies ChannelPort and round-trips a message", async () => {
  const { port1, port2 } = new MessageChannel();
  const a: ChannelPort = port1;
  const b: ChannelPort = port2;
  const got = new Promise<unknown>((resolve) =>
    b.addEventListener("message", (ev) => resolve(ev.data)),
  );
  a.postMessage({ kind: "release", cb: 7 });
  assert.deepStrictEqual(await got, { kind: "release", cb: 7 });
  a.close?.();
  b.close?.();
});

test("a transferred ArrayBuffer is detached on the sending side", async () => {
  const { port1, port2 } = new MessageChannel();
  const got = new Promise<Uint8Array>((resolve) =>
    port2.addEventListener("message", (ev) =>
      resolve((ev as MessageEvent).data),
    ),
  );
  const bytes = new Uint8Array([1, 2, 3]);
  port1.postMessage(bytes, [bytes.buffer]);
  assert.strictEqual(bytes.byteLength, 0);
  assert.deepStrictEqual(Array.from(await got), [1, 2, 3]);
  port1.close();
  port2.close();
});

test("WireError round-trips name and message", () => {
  const e = new RangeError("out of range");
  const w = toWireError(e);
  assert.deepStrictEqual(w, { name: "RangeError", message: "out of range" });
  const back = fromWireError(w);
  assert.strictEqual(back.name, "RangeError");
  assert.strictEqual(back.message, "out of range");
  assert.deepStrictEqual(toWireError("plain string"), {
    name: "Error",
    message: "plain string",
  });
});

test("isChannelMessage rejects foreign shapes", () => {
  assert.ok(isChannelMessage({ kind: "call", id: 1, fn: "f", args: [] }));
  assert.ok(!isChannelMessage(null));
  assert.ok(!isChannelMessage({ kind: "nope" }));
  assert.ok(!isChannelMessage("call"));
});
