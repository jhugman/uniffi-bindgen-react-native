/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  Point,
  Direction,
  Shape,
} from "../../generated/uniffi_value_type_methods";
import { test } from "@/asserts";

// Tests for Point record methods.

test("Point.create() creates a record with given fields", (t) => {
  const p = Point.create({ x: 3.0, y: 4.0 });
  t.assertEqual(p.x, 3.0);
  t.assertEqual(p.y, 4.0);
});

test("Point.new is an alias for Point.create", (t) => {
  const p1 = Point.create({ x: 1.0, y: 2.0 });
  const p2 = Point.new({ x: 1.0, y: 2.0 });
  t.assertEqual(p1.x, p2.x);
  t.assertEqual(p1.y, p2.y);
});

test("Point.distanceTo(self, other) computes distance", (t) => {
  const origin = Point.create({ x: 0.0, y: 0.0 });
  const p = Point.create({ x: 3.0, y: 4.0 });
  const dist = Point.distanceTo(p, origin);
  t.assertEqual(dist, 5.0);
});

test("Point.distanceTo is symmetric", (t) => {
  const a = Point.create({ x: 1.0, y: 2.0 });
  const b = Point.create({ x: 4.0, y: 6.0 });
  t.assertEqual(Point.distanceTo(a, b), Point.distanceTo(b, a));
});

test("Point.scale(self, factor) scales the point", (t) => {
  const p = Point.create({ x: 2.0, y: 3.0 });
  const scaled = Point.scale(p, 2.0);
  t.assertEqual(scaled.x, 4.0);
  t.assertEqual(scaled.y, 6.0);
});

test("Point.scale with factor 0 returns origin", (t) => {
  const p = Point.create({ x: 5.0, y: 7.0 });
  const scaled = Point.scale(p, 0.0);
  t.assertEqual(scaled.x, 0.0);
  t.assertEqual(scaled.y, 0.0);
});

// Tests for Direction flat enum methods.

test("Direction variants are accessible", (t) => {
  t.assertEqual(Direction.North, Direction.North);
  t.assertNotEqual(Direction.North, Direction.South);
});

// Tests for Shape tagged enum (no methods yet, just basic construction).

test("Shape.Circle can be constructed and has tag", (t) => {
  const c = new Shape.Circle({ radius: 5.0 });
  t.assertEqual(c.inner.radius, 5.0);
});

test("Shape.Rectangle can be constructed", (t) => {
  const r = new Shape.Rectangle({ width: 3.0, height: 4.0 });
  t.assertEqual(r.inner.width, 3.0);
  t.assertEqual(r.inner.height, 4.0);
});
