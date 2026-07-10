/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-force-async-list -- jsi
//   cargo test -p uniffi-fixture-force-async-list -- wasm

import {
  AsyncObj,
  SyncObj,
  asyncFn,
  syncFn,
} from "@/generated/uniffi_force_async_list";
import { asyncTest, test } from "@/asserts";

// Members left out of the forceAsync list keep their synchronous surface.
test("un-named object and function are synchronous", (t) => {
  const s = new SyncObj("hi");
  t.assertFalse((s as unknown) instanceof Promise);

  const label = s.label();
  t.assertFalse((label as unknown) instanceof Promise);
  t.assertEqual(label, "hi");

  // Display keeps the name `toString`, so string coercion still works.
  t.assertEqual(s.toString(), "SyncObj(hi)");
  t.assertEqual(`${s}`, "SyncObj(hi)");

  const r = syncFn(41);
  t.assertFalse((r as unknown) instanceof Promise);
  t.assertEqual(r, 42);
});

(async () => {
  await asyncTest("named object and function are async", async (t) => {
    // The primary constructor becomes `static async create()`.
    const pending = AsyncObj.create("hi");
    t.assertTrue((pending as unknown) instanceof Promise);
    // `create` is typed `Promise<AsyncObjLike>`; cast to the class for the
    // renamed trait method. See Risks: "Async constructor returns the interface type".
    const a = (await pending) as AsyncObj;
    t.assertEqual(await a.label(), "hi");
    // Display renames to asyncToString on a forced type.
    t.assertEqual(await a.asyncToString(), "AsyncObj(hi)");

    const fnResult = asyncFn(41);
    t.assertTrue((fnResult as unknown) instanceof Promise);
    t.assertEqual(await fnResult, 42);
    t.end();
  });
})();
