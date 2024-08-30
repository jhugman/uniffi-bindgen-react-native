/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { test } from "../testing/asserts";
import { MyEnum } from "./playground/enums";

test("Enums have private fields", (t) => {
  const v1: MyEnum = new MyEnum.Variant1({ myValue: "string" });
  const v2: MyEnum = new MyEnum.Variant2(42, "string");

  function switchGetTag(v: MyEnum): string {
    switch (v.tag) {
      case "Variant1": {
        const { myValue } = v.inner;
        t.assertEqual(myValue, "string");
        return v.tag;
      }
      case "Variant2": {
        const [p1, p2] = v.inner;
        t.assertEqual(p1, 42);
        t.assertEqual(p2, "string");
        return v.tag;
      }
    }
  }
  switchGetTag(v1);
  switchGetTag(v2);
});
