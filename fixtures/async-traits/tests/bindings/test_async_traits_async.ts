/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-async-traits -- async_wasm

import { getSayAfterTraits } from "@/generated/async_traits";
import { asyncTest } from "@/asserts";
import "@/polyfills";

(async () => {
  await asyncTest("Async trait interface methods", async (t) => {
    const traits = await getSayAfterTraits();

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
    // No remaining-futures check: no JS callbacks here and the Rust count is always 0.
    t.end();
  });
})();
