/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-trait-methods -- async_wasm

import {
  TraitMethods,
  TraitEnum,
  TraitRecord,
  FlatTraitEnum,
  makeFlatTraitEnum,
} from "@/generated/trait_methods";
import { asyncTest } from "@/asserts";

(async () => {
  await asyncTest("toString() is generated", async (t) => {
    const m = (await TraitMethods.create("yo")) as TraitMethods;
    t.assertEqual(await m.asyncToString(), "TraitMethods(yo)");
    // String coercion ("" + m, `${m}`) cannot await, so it is not covered here.
    t.end();
  });

  await asyncTest("equals is generated", async (t) => {
    const m = (await TraitMethods.create("yo")) as TraitMethods;
    const other = (await TraitMethods.create("yo")) as TraitMethods;
    t.assertTrue(await m.equals(other));
    t.end();
  });

  await asyncTest("hashCode is generated", async (t) => {
    const m = (await TraitMethods.create("yo")) as TraitMethods;
    const yoyo = (await TraitMethods.create("yoyo")) as TraitMethods;
    const mHash = await m.hashCode();
    const yoyoHash = await yoyo.hashCode();
    const map = new Map([
      [mHash, 1],
      [yoyoHash, 2],
    ]);
    t.assertEqual(map.get(await m.hashCode()), 1);
    const yoyo2 = (await TraitMethods.create("yoyo")) as TraitMethods;
    t.assertEqual(map.get(await yoyo2.hashCode()), 2);
    t.end();
  });

  await asyncTest("toDebugString() is generated", async (t) => {
    const m = (await TraitMethods.create("yo")) as TraitMethods;
    t.assertEqual(await m.toDebugString(), 'TraitMethods { val: "yo" }');
    t.end();
  });

  await asyncTest("compareTo() is generated", async (t) => {
    const a = (await TraitMethods.create("alpha")) as TraitMethods;
    const b = (await TraitMethods.create("beta")) as TraitMethods;
    t.assertTrue((await a.compareTo(b)) < 0, "alpha should come before beta");
    t.assertTrue((await b.compareTo(a)) > 0, "beta should come after alpha");
    const a2 = (await TraitMethods.create("alpha")) as TraitMethods;
    t.assertEqual(await a.compareTo(a2), 0);
    t.end();
  });

  await asyncTest("enum toString() is generated", async (t) => {
    const a = new TraitEnum.Alpha();
    t.assertEqual(await a.asyncToString(), "Alpha");
    const b = new TraitEnum.Beta({ val: "hello" });
    t.assertEqual(await b.asyncToString(), "Beta(hello)");
    t.end();
  });

  await asyncTest("enum equals() is generated", async (t) => {
    t.assertTrue(await new TraitEnum.Alpha().equals(new TraitEnum.Alpha()));
    t.assertFalse(
      await new TraitEnum.Alpha().equals(new TraitEnum.Beta({ val: "x" })),
    );
    t.end();
  });

  await asyncTest("enum compareTo() is generated", async (t) => {
    t.assertTrue(
      (await new TraitEnum.Alpha().compareTo(
        new TraitEnum.Beta({ val: "x" }),
      )) < 0,
    );
    t.end();
  });

  await asyncTest("record toString() is generated on factory", async (t) => {
    const r = { name: "hello", value: 42 };
    t.assertEqual(await TraitRecord.asyncToString(r), "TraitRecord(hello, 42)");
    t.end();
  });

  await asyncTest("record equals() is generated on factory", async (t) => {
    const a = { name: "x", value: 1 };
    const b = { name: "x", value: 1 };
    const c = { name: "x", value: 2 };
    t.assertTrue(await TraitRecord.equals(a, b));
    t.assertFalse(await TraitRecord.equals(a, c));
    t.end();
  });

  // Tests for FlatTraitEnum: a flat enum (no-data variants) with uniffi trait methods.
  // These verify the namespace merge approach: FlatTraitEnum.Alpha is a plain enum value,
  // while FlatTraitEnum.asyncToString(a) etc. are static namespace functions.

  await asyncTest("flat enum variants are plain values", async (t) => {
    // With namespace approach, variants are plain enum values (not objects/constructors)
    const a = FlatTraitEnum.Alpha;
    const b = FlatTraitEnum.Beta;
    const g = FlatTraitEnum.Gamma;
    t.assertEqual(a, FlatTraitEnum.Alpha);
    t.assertEqual(b, FlatTraitEnum.Beta);
    t.assertEqual(g, FlatTraitEnum.Gamma);
    t.end();
  });

  await asyncTest("flat enum toString() returns Display value", async (t) => {
    t.assertEqual(
      await FlatTraitEnum.asyncToString(FlatTraitEnum.Alpha),
      "alpha",
    );
    t.assertEqual(
      await FlatTraitEnum.asyncToString(FlatTraitEnum.Beta),
      "beta",
    );
    t.assertEqual(
      await FlatTraitEnum.asyncToString(FlatTraitEnum.Gamma),
      "gamma",
    );
    t.end();
  });

  await asyncTest(
    "flat enum toDebugString() returns Debug value",
    async (t) => {
      t.assertEqual(
        await FlatTraitEnum.toDebugString(FlatTraitEnum.Alpha),
        "Alpha",
      );
      t.assertEqual(
        await FlatTraitEnum.toDebugString(FlatTraitEnum.Beta),
        "Beta",
      );
      t.assertEqual(
        await FlatTraitEnum.toDebugString(FlatTraitEnum.Gamma),
        "Gamma",
      );
      t.end();
    },
  );

  await asyncTest("flat enum equals() compares correctly", async (t) => {
    t.assertTrue(
      await FlatTraitEnum.equals(FlatTraitEnum.Alpha, FlatTraitEnum.Alpha),
    );
    t.assertTrue(
      await FlatTraitEnum.equals(FlatTraitEnum.Beta, FlatTraitEnum.Beta),
    );
    t.assertFalse(
      await FlatTraitEnum.equals(FlatTraitEnum.Alpha, FlatTraitEnum.Beta),
    );
    t.assertFalse(
      await FlatTraitEnum.equals(FlatTraitEnum.Beta, FlatTraitEnum.Gamma),
    );
    t.end();
  });

  await asyncTest("flat enum hashCode() returns a bigint", async (t) => {
    const h = await FlatTraitEnum.hashCode(FlatTraitEnum.Alpha);
    t.assertEqual(typeof h, "bigint");
    // Equal values must have equal hash codes
    t.assertEqual(
      await FlatTraitEnum.hashCode(FlatTraitEnum.Alpha),
      await FlatTraitEnum.hashCode(FlatTraitEnum.Alpha),
    );
    // Different values should (very likely) have different hash codes
    t.assertNotEqual(
      await FlatTraitEnum.hashCode(FlatTraitEnum.Alpha),
      await FlatTraitEnum.hashCode(FlatTraitEnum.Beta),
    );
    t.end();
  });

  await asyncTest(
    "flat enum compareTo() orders Alpha < Beta < Gamma",
    async (t) => {
      t.assertTrue(
        (await FlatTraitEnum.compareTo(
          FlatTraitEnum.Alpha,
          FlatTraitEnum.Beta,
        )) < 0,
        "Alpha should be less than Beta",
      );
      t.assertTrue(
        (await FlatTraitEnum.compareTo(
          FlatTraitEnum.Beta,
          FlatTraitEnum.Gamma,
        )) < 0,
        "Beta should be less than Gamma",
      );
      t.assertTrue(
        (await FlatTraitEnum.compareTo(
          FlatTraitEnum.Alpha,
          FlatTraitEnum.Gamma,
        )) < 0,
        "Alpha should be less than Gamma",
      );
      t.assertTrue(
        (await FlatTraitEnum.compareTo(
          FlatTraitEnum.Beta,
          FlatTraitEnum.Alpha,
        )) > 0,
        "Beta should be greater than Alpha",
      );
      t.assertEqual(
        await FlatTraitEnum.compareTo(FlatTraitEnum.Alpha, FlatTraitEnum.Alpha),
        0,
        "Alpha equals Alpha",
      );
      t.end();
    },
  );

  await asyncTest("flat enum roundtrips through FFI", async (t) => {
    const a = await makeFlatTraitEnum(1);
    const b = await makeFlatTraitEnum(2);
    const g = await makeFlatTraitEnum(99);
    t.assertEqual(a, FlatTraitEnum.Alpha);
    t.assertEqual(b, FlatTraitEnum.Beta);
    t.assertEqual(g, FlatTraitEnum.Gamma);
    t.assertEqual(await FlatTraitEnum.asyncToString(a), "alpha");
    t.assertEqual(await FlatTraitEnum.asyncToString(b), "beta");
    t.assertEqual(await FlatTraitEnum.asyncToString(g), "gamma");
    t.end();
  });
})();
