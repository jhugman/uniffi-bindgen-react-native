/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// The host's way to force a full collection. Throws where there is none: a
// test that only waited instead would pass or fail for reasons that have
// nothing to do with the code under test.
function fullCollection(): () => void {
  const g = globalThis as any;
  // The Hermes test-runner installs this.
  if (typeof g.__hermesGc === "function") {
    return g.__hermesGc;
  }
  if (typeof g.gc === "function") {
    return g.gc;
  }
  throw new Error(
    "collectGarbage(): this runtime cannot force a garbage collection. " +
      "The Hermes test-runner provides __hermesGc; " +
      "under node, run with --expose-gc, e.g. NODE_OPTIONS=--expose-gc.",
  );
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

// Collect, then yield to the event loop so that anything the collection queued
// on the JS thread gets to run, several times over: one collection need not
// find everything.
export async function collectGarbage(): Promise<void> {
  const gc = fullCollection();
  for (let i = 0; i < 20; i++) {
    gc();
    await sleep(10);
  }
}
