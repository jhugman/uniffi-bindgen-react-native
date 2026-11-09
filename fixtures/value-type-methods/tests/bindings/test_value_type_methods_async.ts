/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-value-type-methods -- async_wasm

import { Point, Direction, Shape } from "@/generated/uniffi_value_type_methods";
import { asyncTest } from "@/asserts";

// floats should be "close enough".
const almostEquals = (a: number, b: number): boolean =>
  Math.abs(a - b) < 0.000001;

(async () => {
  // Tests for Point record methods.

  await asyncTest(
    "Point.create() creates a record with given fields",
    async (t) => {
      const p = Point.create({ x: 3.0, y: 4.0 });
      t.assertEqual(p.x, 3.0);
      t.assertEqual(p.y, 4.0);
      t.end();
    },
  );

  await asyncTest("Point.new is an alias for Point.create", async (t) => {
    const p1 = Point.create({ x: 1.0, y: 2.0 });
    const p2 = Point.new({ x: 1.0, y: 2.0 });
    t.assertEqual(p1.x, p2.x);
    t.assertEqual(p1.y, p2.y);
    t.end();
  });

  await asyncTest(
    "Point.distanceTo(self, other) computes distance",
    async (t) => {
      const origin = Point.create({ x: 0.0, y: 0.0 });
      const p = Point.create({ x: 3.0, y: 4.0 });
      const dist = await Point.distanceTo(p, origin);
      t.assertEqual(dist, 5.0);
      t.end();
    },
  );

  await asyncTest("Point.distanceTo is symmetric", async (t) => {
    const a = Point.create({ x: 1.0, y: 2.0 });
    const b = Point.create({ x: 4.0, y: 6.0 });
    t.assertEqual(await Point.distanceTo(a, b), await Point.distanceTo(b, a));
    t.end();
  });

  await asyncTest("Point.scale(self, factor) scales the point", async (t) => {
    const p = Point.create({ x: 2.0, y: 3.0 });
    const scaled = await Point.scale(p, 2.0);
    t.assertEqual(scaled.x, 4.0);
    t.assertEqual(scaled.y, 6.0);
    t.end();
  });

  await asyncTest("Point.scale with factor 0 returns origin", async (t) => {
    const p = Point.create({ x: 5.0, y: 7.0 });
    const scaled = await Point.scale(p, 0.0);
    t.assertEqual(scaled.x, 0.0);
    t.assertEqual(scaled.y, 0.0);
    t.end();
  });

  // Tests for Direction flat enum methods.

  await asyncTest("Direction variants are accessible", async (t) => {
    t.assertEqual(Direction.North, Direction.North);
    t.assertNotEqual(Direction.North, Direction.South);
    t.end();
  });

  await asyncTest("Direction.opposite(North) returns South", async (t) => {
    t.assertEqual(await Direction.opposite(Direction.North), Direction.South);
    t.end();
  });

  await asyncTest("Direction.opposite(South) returns North", async (t) => {
    t.assertEqual(await Direction.opposite(Direction.South), Direction.North);
    t.end();
  });

  await asyncTest("Direction.opposite(East) returns West", async (t) => {
    t.assertEqual(await Direction.opposite(Direction.East), Direction.West);
    t.end();
  });

  await asyncTest("Direction.opposite(West) returns East", async (t) => {
    t.assertEqual(await Direction.opposite(Direction.West), Direction.East);
    t.end();
  });

  await asyncTest(
    "Direction.isVertical returns true for North and South",
    async (t) => {
      t.assertTrue(await Direction.isVertical(Direction.North));
      t.assertTrue(await Direction.isVertical(Direction.South));
      t.end();
    },
  );

  await asyncTest(
    "Direction.isVertical returns false for East and West",
    async (t) => {
      t.assertFalse(await Direction.isVertical(Direction.East));
      t.assertFalse(await Direction.isVertical(Direction.West));
      t.end();
    },
  );

  // Tests for Shape tagged enum construction and area method.

  await asyncTest("Shape.Circle can be constructed and has tag", async (t) => {
    const c = new Shape.Circle({ radius: 5.0 });
    t.assertEqual(c.inner.radius, 5.0);
    t.end();
  });

  await asyncTest("Shape.Rectangle can be constructed", async (t) => {
    const r = new Shape.Rectangle({ width: 3.0, height: 4.0 });
    t.assertEqual(r.inner.width, 3.0);
    t.assertEqual(r.inner.height, 4.0);
    t.end();
  });

  await asyncTest("Shape.area of a circle with radius 1 is pi", async (t) => {
    const circle = new Shape.Circle({ radius: 1.0 });
    t.assertEqual(await Shape.area(circle), Math.PI, undefined, almostEquals);
    t.end();
  });

  await asyncTest("Shape.area of a rectangle 3x4 is 12", async (t) => {
    const rect = new Shape.Rectangle({ width: 3.0, height: 4.0 });
    t.assertEqual(await Shape.area(rect), 12.0, undefined, almostEquals);
    t.end();
  });

  await asyncTest(
    "Shape.area scales with radius squared for circles",
    async (t) => {
      const c2 = new Shape.Circle({ radius: 2.0 });
      t.assertEqual(
        await Shape.area(c2),
        4.0 * Math.PI,
        undefined,
        almostEquals,
      );
      t.end();
    },
  );
})();
