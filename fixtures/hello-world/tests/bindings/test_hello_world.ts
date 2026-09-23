/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run (Jsi and Napi are the oracles; Jsi2 is the generic JSI player under test):
//   cargo test -p uniffi-fixture-hello-world -- jsi::
//   cargo test -p uniffi-fixture-hello-world -- napi::
//   cargo test -p uniffi-fixture-hello-world -- jsi2::

import { test } from "@/asserts";
import { add } from "@/generated/hello_world";

test("add returns the sum of two u32s", (t) => {
  t.assertEqual(add(2, 3), 5);
  t.assertEqual(add(0, 0), 0);
});
