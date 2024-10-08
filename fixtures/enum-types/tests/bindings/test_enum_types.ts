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
  AnimalRecord,
  AnimalSignedInt,
  AnimalUInt,
  getAnimal,
  identityEnumWithAssociatedType,
  identityEnumWithNamedAssociatedType,
  CollidingVariants,
  CollidingVariants_Tags,
  identityCollidingVariants,
  AnimalObjectInterface,
} from "../../generated/enum_types";

test("Enum discriminant", (t) => {
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

test("Variant naming cam collide with existing types", (t) => {
  {
    const record = AnimalRecord.create({ value: 5 });
    const variant1 = CollidingVariants.AnimalRecord.new(record);
    const variant2 = new CollidingVariants.AnimalRecord(record);

    t.assertEqual(variant1.tag, CollidingVariants_Tags.AnimalRecord);
    t.assertEqual(variant2.tag, CollidingVariants_Tags.AnimalRecord);
    t.assertEqual(variant1, variant2);
    t.assertEqual(identityCollidingVariants(variant1), variant2);

    t.assertTrue(CollidingVariants.instanceOf(variant1));
    t.assertTrue(CollidingVariants.AnimalRecord.instanceOf(variant1));
  }
  {
    const obj = new AnimalObject(1);
    const variant1 = CollidingVariants.AnimalObject.new(obj);
    const variant2 = new CollidingVariants.AnimalObject(obj);

    t.assertEqual(variant1.tag, CollidingVariants_Tags.AnimalObject);
    t.assertEqual(variant2.tag, CollidingVariants_Tags.AnimalObject);
    t.assertEqual(variant1, variant2);
    t.assertEqual(identityCollidingVariants(variant1), variant2);

    t.assertTrue(CollidingVariants.instanceOf(variant1));
    t.assertTrue(CollidingVariants.AnimalObject.instanceOf(variant1));
  }

  {
    const obj = new AnimalObject(1);
    const variant1 = CollidingVariants.AnimalObjectInterface.new(obj);
    const variant2 = new CollidingVariants.AnimalObjectInterface(obj);

    t.assertEqual(variant1.tag, CollidingVariants_Tags.AnimalObjectInterface);
    t.assertEqual(variant2.tag, CollidingVariants_Tags.AnimalObjectInterface);
    t.assertEqual(variant1, variant2);
    t.assertEqual(identityCollidingVariants(variant1), variant2);

    t.assertTrue(CollidingVariants.instanceOf(variant1));
    t.assertTrue(CollidingVariants.AnimalObjectInterface.instanceOf(variant1));
  }
  {
    const animal = Animal.Dog;
    const variant1 = CollidingVariants.Animal.new(animal);
    const variant2 = new CollidingVariants.Animal(animal);

    t.assertEqual(variant1.tag, CollidingVariants_Tags.Animal);
    t.assertEqual(variant2.tag, CollidingVariants_Tags.Animal);
    t.assertEqual(variant1, variant2);
    t.assertEqual(identityCollidingVariants(variant1), variant2);

    t.assertTrue(CollidingVariants.instanceOf(variant1));
    t.assertTrue(CollidingVariants.Animal.instanceOf(variant1));
  }

  {
    const variant1 = CollidingVariants.CollidingVariants.new();
    const variant2 = new CollidingVariants.CollidingVariants();

    t.assertEqual(variant1.tag, CollidingVariants_Tags.CollidingVariants);
    t.assertEqual(variant2.tag, CollidingVariants_Tags.CollidingVariants);
    t.assertEqual(variant1, variant2);
    t.assertEqual(identityCollidingVariants(variant1), variant2);

    t.assertTrue(CollidingVariants.instanceOf(variant1));
    t.assertTrue(CollidingVariants.CollidingVariants.instanceOf(variant1));
  }
});

// This tests the generated Typescript and serves as an example of how to
// to use enums with values.
//
// In each of the variants, we switch match on the `variant.tag`. Typescript then
// infers the type of `variant.inner`.
//
// In this particular example, each of the variants has a tuple value, of length 1.
// If the types aren't inferred correctly, then Typescript would error at compile time.
function testPatternMatching(variant: CollidingVariants) {
  switch (variant.tag) {
    case CollidingVariants_Tags.AnimalRecord: {
      const record: AnimalRecord = variant.inner[0];
      break;
    }
    case CollidingVariants_Tags.AnimalObject: {
      const object: AnimalObjectInterface = variant.inner[0];
      break;
    }
    case CollidingVariants_Tags.AnimalObjectInterface: {
      const object: AnimalObjectInterface = variant.inner[0];
      break;
    }
    case CollidingVariants_Tags.Animal: {
      const animal: Animal = variant.inner[0];
      break;
    }
  }
}

// This tests the generated Typescript and serves as an example of how to
// to use enums with values.
//
// In each of the variants, we switch match on the `variant.tag`. Typescript then
// infers the type of `variant.inner`.
//
// In this particular example, Cat has no associated variables. Dog an associated tuple
// of length 1, and of type `AnimalObjectInterface`.
function testPatternMatching3(variant: AnimalAssociatedType) {
  switch (variant.tag) {
    case AnimalAssociatedType_Tags.Cat: {
      // const none: undefined = variant.inner;
      break;
    }
    case AnimalAssociatedType_Tags.Dog: {
      const dog: AnimalObjectInterface = variant.inner[0];
      break;
    }
  }
}

// This tests the generated Typescript and serves as an example of how to
// to use enums with values.
//
// In each of the variants, we switch match on the `variant.tag`. Typescript then
// infers the type of `variant.inner`.
//
// In this particular example, Cat has no associated variables. Dog has an object
// with one named value, of type `AnimalObjectInterface`.
function testPatternMatching2(variant: AnimalNamedAssociatedType) {
  switch (variant.tag) {
    case AnimalNamedAssociatedType_Tags.Cat: {
      // const cat: AnimalObjectInterface = variant.inner.value;
      break;
    }
    case AnimalNamedAssociatedType_Tags.Dog: {
      const dog: AnimalObjectInterface = variant.inner.value;
      break;
    }
  }
}
