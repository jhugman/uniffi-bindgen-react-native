/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import myModule, { getSayAfterTraits } from "../../generated/async_traits";
import { asyncTest, Asserts, test } from "@/asserts";
import {
  uniffiRustFutureHandleCount,
  uniffiForeignFutureHandleCount,
} from "uniffi-bindgen-react-native";
import "@/polyfills";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

function checkRemainingFutures(t: Asserts) {
  t.assertEqual(
    0,
    uniffiRustFutureHandleCount(),
    "Number of remaining futures should be zero",
  );
  t.assertEqual(
    0,
    uniffiForeignFutureHandleCount(),
    "Number of remaining foreign futures should be zero",
  );
}

(async () => {
  await asyncTest("Async trait interface methods", async (t) => {
    const traits = getSayAfterTraits();

    await t.asyncMeasure(
      async () => {
        let result1 = await traits[0].sayAfter(300, "Alice");
        let result2 = await traits[1].sayAfter(200, "Bob");

        t.assertEqual(result1, "Hello, Alice!");
        t.assertEqual(result2, "Hello, Bob!");
      },
      500,
      100,
    );
    checkRemainingFutures(t);
    t.end();
  });
})();
