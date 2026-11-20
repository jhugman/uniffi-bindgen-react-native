/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { test } from "node:test";
import { createSender } from "@ubjs/worker";
import {
  serveUtilityWith,
  type ServeUtilityOptions,
  type UtilityDeps,
} from "../src/utility/core.js";
import { fromMessagePortMain } from "../src/utility/port.js";
import { ERROR, READY, type ErrorMessage } from "../src/protocol.js";
import { asMain, fakeMessageChannelMain } from "./fakes/ports.js";
import { tick, within } from "./fakes/helpers.js";
import { DEFS, fakePlayer } from "./fakes/player.js";
import { FakeParentPort, fakeOpen } from "./fakes/utility.js";

function setup(options?: ServeUtilityOptions, open?: UtilityDeps["open"]) {
  const parentPort = new FakeParentPort();
  const chan = fakeMessageChannelMain();
  const { player, state } = fakePlayer();
  const opener = fakeOpen(player);
  const exits: number[] = [];
  let exited!: () => void;
  const hasExited = new Promise<void>((resolve) => (exited = resolve));
  serveUtilityWith(
    {
      parentPort,
      exit: (code) => {
        exits.push(code);
        exited();
      },
      open: open ?? opener.open,
    },
    DEFS,
    options,
  );
  return { parentPort, chan, state, exits, hasExited, ...opener };
}

function assertFailed(
  u: ReturnType<typeof setup>,
  message: string | RegExp,
): void {
  assert.deepStrictEqual(u.exits, [1]);
  assert.strictEqual(u.parentPort.posted.length, 1, "only the error");
  const [error] = u.parentPort.posted as ErrorMessage[];
  assert.strictEqual(typeof error.stack, "string");
  if (typeof message === "string") {
    assert.deepStrictEqual(error, { type: ERROR, message, stack: error.stack });
  } else {
    assert.strictEqual(error.type, ERROR);
    assert.match(error.message, message);
  }
}

test("init opens and registers the library once, then posts ready", async () => {
  const u = setup();
  // Attached at once and kept, so the utility stays up after init.
  assert.strictEqual(u.parentPort.listenerCount("message"), 1);
  u.parentPort.sendInit({}, [u.chan.port1]);
  await tick();

  assert.deepStrictEqual(u.opened, ["/abs/lib.dylib"]);
  assert.strictEqual(u.registers(), 1);
  assert.strictEqual(u.parentPort.posted.length, 1);
  assert.deepStrictEqual(u.parentPort.posted, [{ type: READY }]);
  assert.strictEqual(u.parentPort.listenerCount("message"), 1);
  assert.deepStrictEqual(u.exits, []);
});

test("onInit sees the library and info before the library opens", () => {
  const seen: [unknown, number][] = [];
  const u = setup({
    onInit: (init) => void seen.push([init, u.opened.length]),
  });
  u.parentPort.sendInit({ info: { dataDir: "/data" } }, [u.chan.port1]);

  assert.deepStrictEqual(seen, [
    [{ library: "/abs/lib.dylib", info: { dataDir: "/data" } }, 0],
  ]);
  assert.deepStrictEqual(u.opened, ["/abs/lib.dylib"]);
});

test("a closed port aborts calls in flight and exits 0, without unload", async () => {
  const u = setup();
  u.parentPort.sendInit({}, [u.chan.port1]);
  const sender = createSender(DEFS, fromMessagePortMain(asMain(u.chan.port2)));

  const call = sender.sleep(10_000, { code: 0 }, undefined);
  call.catch(() => {});
  // The port is ordered: once echo answers, the receiver is running sleep.
  await within(sender.echo(new Uint8Array([1])));
  assert.strictEqual(u.state.cancelled, 0);

  u.chan.port2.close();
  await within(u.hasExited);
  assert.strictEqual(u.state.cancelled, 1);
  assert.deepStrictEqual(u.exits, [0]);
});

test("a failure to start posts the error and exits 1", async () => {
  const fails: [string, () => ReturnType<typeof setup>, string][] = [
    [
      "open",
      () =>
        setup(undefined, () => {
          throw new Error("dlopen failed");
        }),
      "dlopen failed",
    ],
    [
      "@ubjs/node",
      () =>
        setup(undefined, () =>
          Promise.reject(
            new Error("Cannot find module '@ubjs/node-darwin-arm64'"),
          ),
        ),
      "Cannot find module '@ubjs/node-darwin-arm64'",
    ],
    [
      "register",
      () =>
        setup(undefined, () => ({
          register() {
            throw new Error("missing symbol uniffi_x_fn_y");
          },
        })),
      "missing symbol uniffi_x_fn_y",
    ],
    [
      "onInit",
      () =>
        setup({
          onInit() {
            throw new TypeError("no logger");
          },
        }),
      "no logger",
    ],
  ];
  for (const [what, make, message] of fails) {
    const u = make();
    u.parentPort.sendInit({}, [u.chan.port1]);
    await tick();
    assertFailed(u, message);
    assert.ok(u.chan.port1.closed, `${what}: its port is closed`);
    if (what === "onInit") assert.deepStrictEqual(u.opened, []);
  }

  // A thrown non-Error is posted as its string, with no stack.
  const u = setup(undefined, () => {
    throw "dlopen said no";
  });
  u.parentPort.sendInit({}, [u.chan.port1]);
  await tick();
  assert.deepStrictEqual(u.exits, [1]);
  assert.deepStrictEqual(u.parentPort.posted, [
    { type: ERROR, message: "dlopen said no" },
  ]);
});

test("other messages and a second init are ignored", async () => {
  const u = setup();
  for (const data of [
    undefined,
    null,
    "init",
    { type: "other" },
    { type: READY },
  ])
    u.parentPort.emit("message", { data, ports: [] });
  assert.deepStrictEqual(u.opened, []);
  assert.deepStrictEqual(u.parentPort.posted, []);

  u.parentPort.sendInit({}, [u.chan.port1]);
  const second = fakeMessageChannelMain().port1;
  u.parentPort.sendInit({}, [second]);
  u.parentPort.sendInit({ library: "/abs/other.dylib" }, [
    fakeMessageChannelMain().port1,
  ]);
  await tick();

  assert.ok(second.closed, "an ignored init's port is closed");
  assert.deepStrictEqual(u.exits, []);
  assert.strictEqual(u.opened.length, 1);
  assert.strictEqual(u.registers(), 1);
  assert.strictEqual(u.parentPort.posted.length, 1, "only the first ready");
});
