/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-js-global-names -- jsi::
//   cargo test -p uniffi-fixture-js-global-names -- napi::
//   cargo test -p uniffi-fixture-js-global-names -- jsi2::

import { test } from "@/asserts";
import {
  apply,
  makeMap,
  objectValue,
  Object_,
  Symbol_,
} from "@/generated/js_global_names";

test("an object named Object is generated as Object_", (t) => {
  const object = new Object_(42);
  t.assertEqual(object.value(), 42);
  t.assertEqual(objectValue(object), 42);
});

test("a record named Map is generated as Map_", (t) => {
  const map = makeMap("answer", 42);
  t.assertEqual(map.key, "answer");
  t.assertEqual(map.value, 42);
});

test("an enum named Symbol is generated as Symbol_", (t) => {
  t.assertEqual(apply(Symbol_.Plus, 40, 2), 42);
  t.assertEqual(apply(Symbol_.Minus, 44, 2), 42);
});

test("the globals are untouched", (t) => {
  t.assertEqual(Object.keys({ a: 1 }).length, 1);
  t.assertEqual(new Map([["a", 1]]).get("a"), 1);
  t.assertEqual(typeof Symbol("s"), "symbol");
});
