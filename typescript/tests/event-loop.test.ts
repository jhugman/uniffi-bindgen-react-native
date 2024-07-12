/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { asyncTest } from "../testing/asserts";

asyncTest("Dummy test that should end properly", async (t) => {
  t.end();
});

// asyncTest("Top level empty test that should error out", async (t) => {});

asyncTest("assertThrowsAsync catches errors", async (t) => {
  await t.assertThrowsAsync("Error.unknown", async () => {
    throw new Error();
  });
  t.end();
});

asyncTest("t.end() ends asynchronously", async (t) => {
  setTimeout(async () => {
    t.end();
  }, 20);
});
