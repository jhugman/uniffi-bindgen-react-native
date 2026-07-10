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
    t.assertEqual(typeof (await w.hashCode()), "bigint");
    const a = await mk("alpha");
    const b = await mk("beta");
    t.assertTrue((await a.compareTo(b)) < 0);
    t.assertEqual(await a.compareTo(await mk("alpha")), 0);
    t.end();
  });

  await asyncTest("Debug-only object gets an async asyncToString delegator", async (t) => {
    const d = (await DebugOnly.create(7)) as DebugOnly;
    // asyncToString delegates to toDebugString; both are async now.
    t.assertEqual(await d.asyncToString(), await d.toDebugString());
    t.end();
  });

  await asyncTest("record namespace trait functions are async", async (t) => {
    // Records stay plain objects; only their trait functions turn async.
    const r: WidgetRecord = { name: "hello", value: 42 };
    t.assertEqual(await WidgetRecord.asyncToString(r), "WidgetRecord(hello, 42)");
    t.assertTrue(await WidgetRecord.equals(r, { name: "hello", value: 42 }));
    t.assertFalse(await WidgetRecord.equals(r, { name: "hello", value: 43 }));
    t.assertEqual(typeof (await WidgetRecord.hashCode(r)), "bigint");
    t.end();
  });

  await asyncTest("tagged enum trait methods + value method are async", async (t) => {
    // Variant construction stays synchronous (pure JS).
    const alpha = new WidgetEnum.Alpha();
    const beta = new WidgetEnum.Beta({ val: "x" });
    t.assertEqual(await alpha.asyncToString(), "Alpha");
    t.assertEqual(await beta.asyncToString(), "Beta(x)");
    t.assertTrue(await alpha.equals(new WidgetEnum.Alpha()));
    t.assertTrue((await alpha.compareTo(beta)) < 0);
    // Namespace value method.
    t.assertEqual(await WidgetEnum.describe(alpha), "alpha");
    t.assertEqual(await WidgetEnum.describe(beta), "beta:x");
    t.end();
  });

  await asyncTest("flat enum namespace functions + top-level fn are async", async (t) => {
    // Variant values stay plain.
    const one = FlatWidget.One;
    t.assertEqual(await FlatWidget.asyncToString(FlatWidget.One), "one");
    t.assertEqual(await FlatWidget.toDebugString(FlatWidget.Two), "Two");
    t.assertTrue(await FlatWidget.equals(FlatWidget.One, one));
    t.assertTrue((await FlatWidget.compareTo(FlatWidget.One, FlatWidget.Two)) < 0);
    t.assertEqual(await FlatWidget.ordinal(FlatWidget.Three), 3);
    // Top-level function is async and round-trips through the FFI.
    const two = await makeFlatWidget(2);
    t.assertEqual(two, FlatWidget.Two);
    t.assertEqual(await FlatWidget.asyncToString(two), "two");
    t.end();
  });
})();
