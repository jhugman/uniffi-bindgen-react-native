/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// An object JS drops without calling uniffiDestroy() is freed in Rust once the
// garbage collector has collected it; one it does destroy is freed only once.
//
// Rust counts every live Coveralls, so each test compares that count with the
// one it started from. A leak leaves it high; a second free of a destroyed
// object would take it low.
import coverall, { Coveralls, getNumAlive } from "@/generated/coverall";
import { asyncTest, type AsyncAsserts } from "@/asserts";
import "@/polyfills";

coverall.initialize();

const N = 100;

// The Hermes test-runner installs this; it forces a full collection.
const hermesGc: () => void = (globalThis as any).__hermesGc;

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

// Collect, then yield to the event loop so that anything the collection queued
// on the JS thread gets to run, several times over: one collection need not
// find everything.
async function collectGarbage() {
  for (let i = 0; i < 20; i++) {
    hermesGc();
    await sleep(10);
  }
}

// Each step makes its garbage in a function of its own, so that nothing on the
// caller's stack still refers to it when the collection runs.
function makeAndDrop() {
  for (let i = 0; i < N; i++) {
    new Coveralls(`dropped-${i}`);
  }
}

function makeAndDestroy() {
  for (let i = 0; i < N; i++) {
    new Coveralls(`destroyed-${i}`).uniffiDestroy();
  }
}

function cloneAndDrop(original: Coveralls) {
  for (let i = 0; i < N; i++) {
    original.cloneMe();
  }
}

function assertAlive(t: AsyncAsserts, expected: bigint, when: string) {
  const alive = getNumAlive();
  t.assertEqual(
    alive,
    expected,
    `${when}: ${alive} Coveralls alive in Rust, expected ${expected}`,
  );
}

(async () => {
  await asyncTest(
    "objects constructed in JS and dropped are freed after a GC",
    async (t) => {
      const before = getNumAlive();
      makeAndDrop();
      assertAlive(t, before + BigInt(N), "before the GC");
      await collectGarbage();
      assertAlive(t, before, "after the GC");
      t.end();
    },
  );

  await asyncTest(
    "objects returned from Rust and dropped are freed after a GC",
    async (t) => {
      const original = new Coveralls("original");
      const before = getNumAlive();
      cloneAndDrop(original);
      assertAlive(t, before + BigInt(N), "before the GC");
      await collectGarbage();
      assertAlive(t, before, "after the GC");
      original.uniffiDestroy();
      t.end();
    },
  );

  await asyncTest(
    "objects destroyed with uniffiDestroy() are not freed again by the GC",
    async (t) => {
      // Held across the collection, so a second free of a destroyed object
      // would show as fewer alive than `before`, not as an underflow.
      const kept = Array.from(
        { length: N },
        (_, i) => new Coveralls(`kept-${i}`),
      );
      const before = getNumAlive();
      makeAndDestroy();
      assertAlive(t, before, "after uniffiDestroy()");
      await collectGarbage();
      assertAlive(t, before, "after the GC");
      kept.forEach((c) => c.uniffiDestroy());
      await collectGarbage();
      assertAlive(t, before - BigInt(N), "after destroying the kept objects");
      t.end();
    },
  );
})();
