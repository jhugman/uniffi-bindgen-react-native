/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import {
  getCustomTypesDemo,
  getMaybeCustomDefault,
  Handle,
  identityEnumWrapper,
  MyEnum_Tags,
  unwrapEnumWrapper,
} from "../../generated/custom_types";
import "@/polyfills";
import { secondsToDate, URL } from "@/converters";
import { test } from "@/asserts";

/// These tests are worth looking at inconjunction with the uniffi.toml file
// in the crate.

test("Rust url::Url --> via String --> Typescript Url", (t) => {
  // Test simple values.
  const demo = getCustomTypesDemo(undefined);
  // Url has an imported classe
  const urlString = "http://example.com/";
  t.assertEqual(demo.url.toString(), urlString);
  t.assertEqual(demo.url, new URL(urlString));

  demo.url = new URL("http://new.example.com/");
  t.assertEqual(demo, getCustomTypesDemo(demo));
});

test("Rust Handle struct --> via u64/BigInt --> BigInt", (t) => {
  const demo = getCustomTypesDemo(undefined);
  t.assertEqual(demo.handle, BigInt("123"));

  // Handle is type aliases to BigInt.
  const handle1: bigint = demo.handle;
  const handle2: Handle = demo.handle;

  demo.handle = BigInt("456");
  t.assertEqual(demo, getCustomTypesDemo(demo));
});

test("Rust TimeInterval structs --> via u64/BigInt --> Date", (t) => {
  const demo = getCustomTypesDemo(undefined);

  t.assertEqual(demo.timeIntervalMs, new Date(456000));
  t.assertEqual(demo.timeIntervalSecDbl, secondsToDate(456.0));
  t.assertEqual(demo.timeIntervalSecFlt, secondsToDate(777.0));

  demo.timeIntervalMs = new Date(789.0);
  demo.timeIntervalSecDbl = new Date(789.0);
  demo.timeIntervalSecFlt = new Date(111.0);
  t.assertEqual(demo, getCustomTypesDemo(demo));
});

test("custom type optional parameter with default works", (t) => {
  // Should be callable without arguments (defaults to None/undefined)
  t.assertNull(getMaybeCustomDefault(undefined));
  // Should work when provided
  // Note: Handle is i64, in TypeScript it's BigInt
  t.assertEqual(getMaybeCustomDefault(BigInt(42)), BigInt(42));
});

/// Custom types can be made of generated types.
test("Rust EnumWrapper structs --> via MyEnum --> string", (t) => {
  const strings = ["A", "dAtA", "Alice", "Bob", "Charlie"];
  for (const s of strings) {
    t.assertEqual(s, identityEnumWrapper(s));
    const enumValue = unwrapEnumWrapper(s);
    switch (enumValue.tag) {
      case MyEnum_Tags.A: {
        const value = enumValue.inner[0];
        t.assertEqual(value, s);
        t.assertTrue(value.indexOf("A") >= 0);
        break;
      }
      case MyEnum_Tags.B: {
        const value = enumValue.inner[0];
        t.assertEqual(value, s);
        t.assertFalse(value.indexOf("A") >= 0);
        break;
      }
    }
  }
});
