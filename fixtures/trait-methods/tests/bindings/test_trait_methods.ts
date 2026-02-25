/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { TraitMethods, TraitEnum, TraitRecord, FlatTraitEnum, makeFlatTraitEnum } from "../../generated/trait_methods";
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

test("compareTo() is generated", (t) => {
  const a = new TraitMethods("alpha");
  const b = new TraitMethods("beta");
  t.assertTrue(a.compareTo(b) < 0, "alpha should come before beta");
  t.assertTrue(b.compareTo(a) > 0, "beta should come after alpha");
  t.assertEqual(a.compareTo(new TraitMethods("alpha")), 0);
});

test("enum toString() is generated", (t) => {
  const a = new TraitEnum.Alpha();
  t.assertEqual(a.toString(), "Alpha");
  const b = new TraitEnum.Beta({ val: "hello" });
  t.assertEqual(b.toString(), "Beta(hello)");
});

test("enum equals() is generated", (t) => {
  t.assertTrue(new TraitEnum.Alpha().equals(new TraitEnum.Alpha()));
  t.assertFalse(new TraitEnum.Alpha().equals(new TraitEnum.Beta({ val: "x" })));
});

test("enum compareTo() is generated", (t) => {
  t.assertTrue(new TraitEnum.Alpha().compareTo(new TraitEnum.Beta({ val: "x" })) < 0);
});

test("record toString() is generated on factory", (t) => {
  const r = { name: "hello", value: 42 };
  t.assertEqual(TraitRecord.toString(r), "TraitRecord(hello, 42)");
});

test("record equals() is generated on factory", (t) => {
  const a = { name: "x", value: 1 };
  const b = { name: "x", value: 1 };
  const c = { name: "x", value: 2 };
  t.assertTrue(TraitRecord.equals(a, b));
  t.assertFalse(TraitRecord.equals(a, c));
});

// Tests for FlatTraitEnum: a flat enum (no-data variants) with uniffi trait methods.
// These verify the namespace merge approach: FlatTraitEnum.Alpha is a plain enum value,
// while FlatTraitEnum.toString(a) etc. are static namespace functions.

test("flat enum variants are plain values", (t) => {
  // With namespace approach, variants are plain enum values (not objects/constructors)
  const a = FlatTraitEnum.Alpha;
  const b = FlatTraitEnum.Beta;
  const g = FlatTraitEnum.Gamma;
  t.assertEqual(a, FlatTraitEnum.Alpha);
  t.assertEqual(b, FlatTraitEnum.Beta);
  t.assertEqual(g, FlatTraitEnum.Gamma);
});

test("flat enum toString() returns Display value", (t) => {
  t.assertEqual(FlatTraitEnum.toString(FlatTraitEnum.Alpha), "alpha");
  t.assertEqual(FlatTraitEnum.toString(FlatTraitEnum.Beta), "beta");
  t.assertEqual(FlatTraitEnum.toString(FlatTraitEnum.Gamma), "gamma");
});

test("flat enum toDebugString() returns Debug value", (t) => {
  t.assertEqual(FlatTraitEnum.toDebugString(FlatTraitEnum.Alpha), "Alpha");
  t.assertEqual(FlatTraitEnum.toDebugString(FlatTraitEnum.Beta), "Beta");
  t.assertEqual(FlatTraitEnum.toDebugString(FlatTraitEnum.Gamma), "Gamma");
});

test("flat enum equals() compares correctly", (t) => {
  t.assertTrue(FlatTraitEnum.equals(FlatTraitEnum.Alpha, FlatTraitEnum.Alpha));
  t.assertTrue(FlatTraitEnum.equals(FlatTraitEnum.Beta, FlatTraitEnum.Beta));
  t.assertFalse(FlatTraitEnum.equals(FlatTraitEnum.Alpha, FlatTraitEnum.Beta));
  t.assertFalse(FlatTraitEnum.equals(FlatTraitEnum.Beta, FlatTraitEnum.Gamma));
});

test("flat enum hashCode() returns a bigint", (t) => {
  const h = FlatTraitEnum.hashCode(FlatTraitEnum.Alpha);
  t.assertEqual(typeof h, "bigint");
  // Equal values must have equal hash codes
  t.assertEqual(FlatTraitEnum.hashCode(FlatTraitEnum.Alpha), FlatTraitEnum.hashCode(FlatTraitEnum.Alpha));
  // Different values should (very likely) have different hash codes
  t.assertNotEqual(FlatTraitEnum.hashCode(FlatTraitEnum.Alpha), FlatTraitEnum.hashCode(FlatTraitEnum.Beta));
});

test("flat enum compareTo() orders Alpha < Beta < Gamma", (t) => {
  t.assertTrue(FlatTraitEnum.compareTo(FlatTraitEnum.Alpha, FlatTraitEnum.Beta) < 0, "Alpha should be less than Beta");
  t.assertTrue(FlatTraitEnum.compareTo(FlatTraitEnum.Beta, FlatTraitEnum.Gamma) < 0, "Beta should be less than Gamma");
  t.assertTrue(FlatTraitEnum.compareTo(FlatTraitEnum.Alpha, FlatTraitEnum.Gamma) < 0, "Alpha should be less than Gamma");
  t.assertTrue(FlatTraitEnum.compareTo(FlatTraitEnum.Beta, FlatTraitEnum.Alpha) > 0, "Beta should be greater than Alpha");
  t.assertEqual(FlatTraitEnum.compareTo(FlatTraitEnum.Alpha, FlatTraitEnum.Alpha), 0, "Alpha equals Alpha");
});

test("flat enum roundtrips through FFI", (t) => {
  const a = makeFlatTraitEnum(1);
  const b = makeFlatTraitEnum(2);
  const g = makeFlatTraitEnum(99);
  t.assertEqual(a, FlatTraitEnum.Alpha);
  t.assertEqual(b, FlatTraitEnum.Beta);
  t.assertEqual(g, FlatTraitEnum.Gamma);
  t.assertEqual(FlatTraitEnum.toString(a), "alpha");
  t.assertEqual(FlatTraitEnum.toString(b), "beta");
  t.assertEqual(FlatTraitEnum.toString(g), "gamma");
});
