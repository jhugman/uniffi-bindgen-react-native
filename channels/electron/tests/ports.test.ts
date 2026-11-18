/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { once } from "node:events";
import { test } from "node:test";
import { createReceiver, createSender } from "@ubjs/worker";
import { fromMessagePortMain } from "../src/utility/port.js";
import { wrapPagePort } from "../src/page/port.js";
import {
  FakePagePort,
  asMain,
  fakeMessageChannelMain,
  fakeRendererToUtility,
} from "./fakes/ports.js";
import { tick, within } from "./fakes/helpers.js";
import { DEFS, fakePlayer } from "./fakes/player.js";

const MiB8 = 8 * 1024 * 1024;
const asPage = (p: FakePagePort) => p as unknown as MessagePort;
const closedError = { name: "ChannelClosedError" };

test("fromMessagePortMain drops the transfer list", async () => {
  const { port1, port2 } = fakeMessageChannelMain();
  const bytes = new Uint8Array([1, 2, 3]);
  assert.throws(
    () => port1.postMessage(bytes, [bytes.buffer]),
    /Port at index 0 is not a valid port/,
  );

  const port = fromMessagePortMain(asMain(port1));
  port2.start();
  const arrived = once(port2, "message");
  port.postMessage(bytes, [bytes.buffer]);
  const [ev] = await within(arrived);
  assert.deepStrictEqual(ev.data, new Uint8Array([1, 2, 3]));
});

test("its closed settles when the peer closes, and postMessage does nothing after", async () => {
  const { port1, port2 } = fakeMessageChannelMain();
  const port = fromMessagePortMain(asMain(port1));
  let settled = false;
  port.closed.then(() => (settled = true));
  await tick();
  assert.strictEqual(settled, false);

  port2.close();
  await within(port.closed);
  const posted = port1.posted;
  port.postMessage("late");
  assert.strictEqual(port1.posted, posted);
});

test("its closed settles on close()", async () => {
  const { port1, port2 } = fakeMessageChannelMain();
  const port = fromMessagePortMain(asMain(port1));
  const peerHeard = once(port2, "close");
  port.close();
  await within(port.closed);
  assert.strictEqual(port1.closed, true);
  await within(peerHeard);
  port.postMessage("late");
  assert.strictEqual(port1.posted, 0);
});

test("wrapPagePort drops the transfer list, so the caller's buffer survives", async () => {
  const { page, utility } = fakeRendererToUtility();
  const { port } = wrapPagePort(asPage(page));
  utility.start();
  const arrived = once(utility, "message");
  const bytes = new Uint8Array([1, 2, 3]);
  port.postMessage(bytes, [bytes.buffer]);
  const [ev] = await within(arrived);
  assert.deepStrictEqual(ev.data, new Uint8Array([1, 2, 3]));
  assert.strictEqual(bytes.byteLength, 3);
});

test("close() settles closed as { code: null, reason: 'closed' } and closes the raw port", async () => {
  const page = new FakePagePort();
  const { port } = wrapPagePort(asPage(page));
  port.close();
  assert.deepStrictEqual(await within(port.closed), {
    code: null,
    reason: "closed",
  });
  assert.strictEqual(page.isClosed, true);
});

test("the first disconnect wins, and postMessage does nothing after", async () => {
  const page = new FakePagePort();
  const { port, disconnect } = wrapPagePort(asPage(page));
  disconnect({ code: 9, reason: "utility-exited" });
  port.close();
  disconnect({ code: 1, reason: "later" });
  assert.deepStrictEqual(await within(port.closed), {
    code: 9,
    reason: "utility-exited",
  });
  assert.strictEqual(page.isClosed, true);
  port.postMessage("late");
  assert.strictEqual(page.posted, 0);
});
