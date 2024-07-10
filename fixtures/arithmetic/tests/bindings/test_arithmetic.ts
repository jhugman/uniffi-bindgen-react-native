/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// fixture=arithmetic
// cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
// cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}

import * as rust from "../../generated/arithmetic";
import { test } from "@/asserts";
import { console } from "@/hermes";

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
