/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-ext-types -- napi

import { test } from "@/asserts";

import bindings, {
  uniffiInitAsync,
  CombinedType,
  UniffiOneEnum,
  UniffiOneType,
  getCombinedType,
  getUniffiOneEnum,
} from "@/generated";

import "@/polyfills";

// Idempotence: calling uniffiInitAsync twice must not throw.
await uniffiInitAsync();
await uniffiInitAsync();

test("namespaced default exposes all 5 namespaces", (t) => {
  t.assertNotNull(bindings.custom_types);
  t.assertNotNull(bindings.ext_types_custom);
  t.assertNotNull(bindings.imported_types_lib);
  t.assertNotNull(bindings.imported_types_sublib);
  t.assertNotNull(bindings.uniffi_one_ns);
});

test("call a function via the namespaced default", (t) => {
  // imported_types_lib.getCombinedType is exposed as a named export of
  // imported_types_lib.ts and therefore appears on the namespace object.
  const ct = bindings.imported_types_lib.getCombinedType(undefined);
  t.assertEqual(ct.uot.sval, "hello");
});

test("call a function via re-exported name (export * across 5 namespaces)", (t) => {
  const ct: CombinedType = getCombinedType(undefined);
  t.assertEqual(ct.uot.sval, "hello");

  // Pull a value from a different namespace (uniffi_one_ns) via re-export
  // to prove all 5 namespaces flatten without collision.
  t.assertEqual(getUniffiOneEnum(UniffiOneEnum.One), UniffiOneEnum.One);
});

test("UniffiOneType from re-export round-trips", (t) => {
  const v = UniffiOneType.create({ sval: "world" });
  // round-trip via the namespaced default
  const back = bindings.imported_types_lib.getUniffiOneType(v);
  t.assertEqual(back.sval, "world");
});
