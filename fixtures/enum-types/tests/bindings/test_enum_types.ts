/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "@/asserts";
import {
  Animal,
  AnimalAssociatedType,
  AnimalAssociatedType_Tags,
  AnimalLargeUInt,
  AnimalNamedAssociatedType,
  AnimalNamedAssociatedType_Tags,
  AnimalNoReprInt,
  AnimalObject,
  AnimalSignedInt,
  AnimalUInt,
  getAnimal,
  identityEnumWithAssociatedType,
  identityEnumWithNamedAssociatedType,
} from "../../generated/enum_types";

test("Enum disriminant", (t) => {
  t.assertEqual(Animal.Dog, 0);
  t.assertEqual(Animal.Cat, 1);
  t.assertEqual(getAnimal(undefined), Animal.Dog);
  t.assertEqual(AnimalNoReprInt.Dog, 0);
  t.assertEqual(AnimalNoReprInt.Cat, 1);
  t.assertEqual(AnimalUInt.Dog, 3);
  t.assertEqual(AnimalUInt.Cat, 4);
  t.assertEqual(
    AnimalLargeUInt.Dog,
    (BigInt("4294967295") + BigInt("3")).toString(),
  );
  t.assertEqual(
    AnimalLargeUInt.Cat,
    (BigInt("4294967295") + BigInt("4")).toString(),
  );
  t.assertEqual(AnimalSignedInt.Dog, -3);
  t.assertEqual(AnimalSignedInt.Cat, -2);
  t.assertEqual(AnimalSignedInt.Koala, -1);
  t.assertEqual(AnimalSignedInt.Wallaby, 0);
  t.assertEqual(AnimalSignedInt.Wombat, 1);
});

test("Roundtripping enums with values", (t) => {
  function assertEqual(
    left: AnimalAssociatedType,
    right: AnimalAssociatedType,
  ): void {
    t.assertEqual(left.tag, right.tag);
    switch (left.tag) {
      case AnimalAssociatedType_Tags.Cat:
        return;
      case AnimalAssociatedType_Tags.Dog:
        if (AnimalAssociatedType.Dog.instanceOf(right)) {
          t.assertEqual(left.inner[0].record(), right.inner[0].record());
        } else {
          t.fail(`${right} is not a Dog`);
        }
    }
  }
  const values = [
    AnimalAssociatedType.Cat.new(),
    AnimalAssociatedType.Dog.new(new AnimalObject(1)),
    AnimalAssociatedType.Dog.new(new AnimalObject(2)),
  ];

  for (const v of values) {
    t.assertTrue(AnimalAssociatedType.instanceOf(v as any));
    assertEqual(v, identityEnumWithAssociatedType(v));
  }
});

test("Roundtripping enums with name values", (t) => {
  function assertEqual(
    left: AnimalNamedAssociatedType,
    right: AnimalNamedAssociatedType,
  ): void {
    t.assertEqual(left.tag, right.tag);
    switch (left.tag) {
      case AnimalNamedAssociatedType_Tags.Cat:
        return;
      case AnimalNamedAssociatedType_Tags.Dog:
        if (AnimalNamedAssociatedType.Dog.instanceOf(right)) {
          t.assertEqual(left.inner.value.record(), right.inner.value.record());
        } else {
          t.fail(`${right} is not a Dog`);
        }
    }
  }
  const values = [
    AnimalNamedAssociatedType.Cat.new(),
    AnimalNamedAssociatedType.Dog.new({ value: new AnimalObject(1) }),
    AnimalNamedAssociatedType.Dog.new({ value: new AnimalObject(2) }),
  ];

  for (const v of values) {
    t.assertTrue(AnimalNamedAssociatedType.instanceOf(v as any));
    assertEqual(v, identityEnumWithNamedAssociatedType(v));
  }
});
