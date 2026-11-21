/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// AsyncElectron only: a page and its utility process through a reload, a
// utility killed under a live page, and a window close. The window loads this
// script three times; each load runs the next part.
import {
  afterThisPage,
  asyncTest,
  connection,
  exitOf,
  fixture,
  killUtility,
  pageIndex,
  utilityPid,
  type AsyncAsserts,
} from "@/asserts";
import {
  Counter,
  liveCounters,
  liveFutures,
  sleepForever,
} from "@/generated/uniffi_async_channel";

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// A Counter the next load's fresh utility must not still hold.
const keep: unknown[] = [];

async function errorOf(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (e) {
    return e;
  }
  throw new Error("resolved, but should have rejected");
}

const nameOf = (e: unknown) => (e as { name?: unknown } | null)?.name;

async function firstLoad(t: AsyncAsserts): Promise<void> {
  t.assertTrue(fixture.sandboxed, "the page is sandboxed");
  t.assertTrue(fixture.contextIsolated, "the page is context-isolated");
  // By name, as the shipped preload must expose it.
  const bridge = Reflect.get(window, "__ubjsElectron") as
    | { protocol?: unknown }
    | undefined;
  t.assertEqual(bridge?.protocol, 1, "the shipped preload's bridge");
  const c = await Counter.create(1n);
  keep.push(c);
  t.assertEqual(await c.add(1n), 2n);
  afterThisPage("reload");
}

async function afterReload(t: AsyncAsserts): Promise<void> {
  // The bootstrap waited for this exit before it connected, so main had not
  // retired the old utility: it ended itself when its port closed.
  const old = await exitOf(await utilityPid(0));
  t.assertEqual(old.code, 0, "the old utility's exit code");
  t.assertEqual(await liveCounters(), 0, "a fresh utility");
  const early = await Promise.race([
    connection().disconnected,
    sleep(1000).then(() => undefined),
  ]);
  t.assertEqual(early, undefined, "the old utility's exit reached this page");

  // Killed under a call in flight, from outside @ubjs/electron.
  const forever = errorOf(sleepForever());
  while ((await liveFutures()) !== 1) await sleep(20);
  await killUtility();
  t.assertEqual(nameOf(await forever), "ChannelClosedError");
  const info = await connection().disconnected;
  t.assertEqual(info.reason, "utility-exited");
  t.assertEqual(nameOf(await errorOf(liveCounters())), "ChannelClosedError");
  afterThisPage("reload");
}

async function afterKill(t: AsyncAsserts): Promise<void> {
  t.assertEqual(await liveCounters(), 0, "a fresh utility after the kill");
  // Main closes the window, and checks that this utility ends.
  afterThisPage("close");
}

asyncTest(
  "the page and its utility through reloads, a kill and a close",
  async (t) => {
    const parts = [firstLoad, afterReload, afterKill];
    await parts[await pageIndex()](t);
    t.end();
  },
);
