/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-ext-types -- napi

import { asyncTest, test } from "@/asserts";

import bindings, {
  uniffiInitAsync,
  CombinedType,
  SharedEnum,
  SharedRecord,
  getCombinedType,
  getSharedEnum,
} from "@/generated";

import "@/polyfills";

// Idempotence: calling uniffiInitAsync twice must not throw. Not a top-level
// await: the jsi bundle path compiles to es5, which has no such thing.
asyncTest("uniffiInitAsync is idempotent", async (t) => {
  await uniffiInitAsync();
  await uniffiInitAsync();
  t.end();
});

test("namespaced default exposes all 4 namespaces", (t) => {
  t.assertEqual((bindings as Record<string, unknown>).custom_types, undefined);
  t.assertNotNull(bindings.ext_types);
  t.assertNotNull(bindings.ext_types_custom);
  t.assertNotNull(bindings.ext_types_consumer);
  t.assertNotNull(bindings.ext_types_shared);
});

test("call a function via the namespaced default", (t) => {
  // ext_types.getCombinedType is exposed as a named export of
  // ext_types.ts and therefore appears on the namespace object.
  const ct = bindings.ext_types.getCombinedType(undefined);
  t.assertEqual(ct.sharedRecord.sval, "hello");
});

test("call a function via re-exported name (export * across 4 namespaces)", (t) => {
  const ct: CombinedType = getCombinedType(undefined);
  t.assertEqual(ct.sharedRecord.sval, "hello");

  // Pull a value from a different namespace (ext_types_shared) via re-export
  // to prove all 5 namespaces flatten without collision.
  t.assertEqual(getSharedEnum(SharedEnum.One), SharedEnum.One);
});

test("SharedRecord from re-export round-trips", (t) => {
  const v = SharedRecord.create({ sval: "world" });
  // round-trip via the namespaced default
  const back = bindings.ext_types.getSharedRecord(v);
  t.assertEqual(back.sval, "world");
});
