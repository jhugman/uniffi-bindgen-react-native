/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-example-arithmetic -- jsi
//   cargo test -p uniffi-example-arithmetic -- wasm

import * as rust from "@/generated/arithmetic";
import { test } from "@/asserts";
import "@/polyfills";

const a = BigInt(39);
const b = BigInt(3);

test("add", (t) => {
  console.log(`${a} + ${b} = ${rust.add(a, b)}`);
  t.assertEqual(a + b, rust.add(a, b));
});
test("sub", (t) => {
  t.assertEqual(a - b, rust.sub(a, b));
});
test("div", (t) => {
  t.assertEqual(a / b, rust.div(a, b));
});
