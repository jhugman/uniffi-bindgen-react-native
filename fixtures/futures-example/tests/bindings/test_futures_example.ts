/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { sayAfter } from "../../generated/uniffi_example_futures";
import { asyncTest } from "@/asserts";
import "@/polyfills";

asyncTest("sayAfter once", async (t): Promise<void> => {
  const result = await sayAfter(BigInt("500"), "World");
  t.assertEqual(result, `Hello, World!`);
  t.end();
});

asyncTest("sayAfter multiple times", async (t): Promise<void> => {
  const strings = ["World", "Async", "Test", "Rust"];
  for (const s of strings) {
    const result = await sayAfter(BigInt("100"), s);
    t.assertEqual(result, `Hello, ${s}!`);
  }
  t.end();
});
