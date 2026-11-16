/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "../testing/polyfills";
import { asyncTest } from "../testing/asserts";
import { uniffiIgnoreVoidResult } from "../src/index";

async function logged(result: Promise<unknown>, what: string) {
  const lines: unknown[][] = [];
  const original = console.error;
  console.error = (...args: unknown[]) => {
    lines.push(args);
  };
  try {
    uniffiIgnoreVoidResult(result, what);
    await new Promise((r) => setTimeout(r, 0));
  } finally {
    console.error = original;
  }
  return lines;
}

(async () => {
  await asyncTest(
    "a rejection from a closed channel is not logged",
    async (t) => {
      const e = new Error("closed");
      e.name = "ChannelClosedError";
      const lines = await logged(Promise.reject(e), "Counter: free");
      t.assertEqual(lines.length, 0);
      t.end();
    },
  );

  await asyncTest(
    "any other rejection is logged with what failed",
    async (t) => {
      const lines = await logged(
        Promise.reject(new Error("boom")),
        "Counter: free",
      );
      t.assertEqual(lines.length, 1);
      t.assertTrue(String(lines[0][0]).includes("Counter: free failed"));
      t.end();
    },
  );
})();
