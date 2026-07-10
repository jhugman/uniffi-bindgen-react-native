/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-force-async -- jsi
//   cargo test -p uniffi-fixture-force-async -- wasm

import {
  Widget,
  DebugOnly,
  WidgetRecord,
  WidgetEnum,
  FlatWidget,
  makeFlatWidget,
} from "@/generated/uniffi_force_async";
import { asyncTest } from "@/asserts";

(async () => {
  await asyncTest("object surface is async and behavior-preserving", async (t) => {
    // Primary constructor -> `static async create(...)`, which is typed
    // `Promise<WidgetLike>` (the object *interface*). Trait methods
    // (equals/hashCode/compareTo/asyncToString) are members of the *class*,
    // not the interface, so cast to `Widget` to reach them. See Risks:
    // "Async constructor returns the interface type".
    const mk = async (s: string): Promise<Widget> => (await Widget.create(s)) as Widget;
    const w = await mk("yo");
    t.assertEqual(await w.label(), "yo");
    t.assertEqual(await w.asyncToString(), "Widget(yo)");
    t.assertEqual(await w.toDebugString(), 'Widget { val: "yo" }');
    t.assertTrue(await w.equals(await mk("yo")));
    t.assertFalse(await w.equals(await mk("no")));
    // Hash values are not stable across builds, so assert the invariant —
    // equal instances hash equal — rather than a literal.
    t.assertEqual(await w.hashCode(), await (await mk("yo")).hashCode());
    const a = await mk("alpha");
    const b = await mk("beta");
    t.assertTrue((await a.compareTo(b)) < 0);
    t.assertEqual(await a.compareTo(await mk("alpha")), 0);
    t.end();
  });

  await asyncTest(
    "Debug-only object gets an async asyncToString delegator",
    async (t) => {
      const d = (await DebugOnly.create(7)) as DebugOnly;
      const debugStr = await d.toDebugString();
      t.assertEqual(debugStr, "DebugOnly { n: 7 }");
      // With Debug but no Display, asyncToString delegates to toDebugString.
      t.assertEqual(await d.asyncToString(), debugStr);
      t.end();
    },
  );

  await asyncTest("record namespace trait functions are async", async (t) => {
    // Records stay plain objects; only their trait functions turn async.
    const r: WidgetRecord = { name: "hello", value: 42 };
    t.assertEqual(
      await WidgetRecord.asyncToString(r),
      "WidgetRecord(hello, 42)",
    );
    t.assertEqual(
      await WidgetRecord.toDebugString(r),
      'WidgetRecord { name: "hello", value: 42 }',
    );
    t.assertTrue(await WidgetRecord.equals(r, { name: "hello", value: 42 }));
    t.assertFalse(await WidgetRecord.equals(r, { name: "hello", value: 43 }));
    t.assertEqual(
      await WidgetRecord.hashCode(r),
      await WidgetRecord.hashCode({ name: "hello", value: 42 }),
    );
    // Derived Ord compares fields in declaration order: name, then value.
    t.assertEqual(
      await WidgetRecord.compareTo(r, { name: "hello", value: 42 }),
      0,
    );
    t.assertTrue(
      (await WidgetRecord.compareTo(r, { name: "hello", value: 100 })) < 0,
    );
    t.assertTrue(
      (await WidgetRecord.compareTo(r, { name: "zzz", value: 1 })) < 0,
    );
    t.end();
  });

  await asyncTest(
    "tagged enum trait methods + value method are async",
    async (t) => {
      // Variant construction stays synchronous (pure JS).
      const alpha = new WidgetEnum.Alpha();
      const beta = new WidgetEnum.Beta({ val: "x" });
      t.assertEqual(await alpha.asyncToString(), "Alpha");
      t.assertEqual(await beta.asyncToString(), "Beta(x)");
      t.assertEqual(await alpha.toDebugString(), "Alpha");
      t.assertEqual(await beta.toDebugString(), 'Beta { val: "x" }');
      t.assertTrue(await alpha.equals(new WidgetEnum.Alpha()));
      t.assertTrue((await alpha.compareTo(beta)) < 0);
      t.assertEqual(
        await alpha.hashCode(),
        await new WidgetEnum.Alpha().hashCode(),
      );
      // Namespace value method.
      t.assertEqual(await WidgetEnum.describe(alpha), "alpha");
      t.assertEqual(await WidgetEnum.describe(beta), "beta:x");
      t.end();
    },
  );

  await asyncTest(
    "flat enum namespace functions + top-level fn are async",
    async (t) => {
      // Variant values stay plain.
      const one = FlatWidget.One;
      t.assertEqual(await FlatWidget.asyncToString(FlatWidget.One), "one");
      t.assertEqual(await FlatWidget.toDebugString(FlatWidget.Two), "Two");
      t.assertTrue(await FlatWidget.equals(FlatWidget.One, one));
      t.assertEqual(
        await FlatWidget.hashCode(FlatWidget.One),
        await FlatWidget.hashCode(one),
      );
      t.assertTrue(
        (await FlatWidget.compareTo(FlatWidget.One, FlatWidget.Two)) < 0,
      );
      t.assertEqual(await FlatWidget.ordinal(FlatWidget.Three), 3);
      // Top-level function is async and round-trips through the FFI.
      const two = await makeFlatWidget(2);
      t.assertEqual(two, FlatWidget.Two);
      t.assertEqual(await FlatWidget.asyncToString(two), "two");
      t.end();
    },
  );
})();
