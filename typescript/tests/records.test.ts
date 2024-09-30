/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { Asserts, test, xtest } from "../testing/asserts";
import { console } from "../testing/hermes";
import { MyRecord } from "./playground/records";

test("Allow defaults to be missing", (t) => {
  const v: MyRecord = MyRecord.create({
    number: 42,
    bool: true,
    optionalBool: undefined,
  });
  t.assertEqual(v.string, "default");
  t.assertEqual(v.optionalString, undefined);
  t.assertEqual(v.number, 42);
  t.assertEqual(v.bool, true);
  t.assertEqual(v.optionalBool, undefined);
});

test("Allow defaults to be overridden", (t) => {
  const v: MyRecord = MyRecord.create({
    string: "overridden",
    optionalString: "also overridden",
    number: 43,
    bool: false,
    optionalBool: true,
  });

  t.assertEqual(v.string, "overridden");
  t.assertEqual(v.optionalString, "also overridden");
  t.assertEqual(v.number, 43);
  t.assertEqual(v.bool, false);
  t.assertEqual(v.optionalBool, true);
});
