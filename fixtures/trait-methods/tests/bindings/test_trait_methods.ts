/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { TraitMethods } from "../../generated/trait_methods";
import { test } from "@/asserts";

test("toString() is generated", (t) => {
  const m = new TraitMethods("yo");
  t.assertEqual(m.toString(), "TraitMethods(yo)");
  t.assertEqual("" + m, "TraitMethods(yo)");
  t.assertEqual(`${m}`, "TraitMethods(yo)");
});

test("equals is generated", (t) => {
  const m = new TraitMethods("yo");
  t.assertTrue(m.equals(new TraitMethods("yo")));
});

test("hashCode is generated", (t) => {
  const m = new TraitMethods("yo");
  const map = new Map([
    [m.hashCode(), 1],
    [new TraitMethods("yoyo").hashCode(), 2],
  ]);
  t.assertEqual(map.get(m.hashCode()), 1);
  t.assertEqual(map.get(new TraitMethods("yoyo").hashCode()), 2);
});

test("toDebugString() is generated", (t) => {
  const m = new TraitMethods("yo");
  t.assertEqual(m.toDebugString(), 'TraitMethods { val: "yo" }');
});
