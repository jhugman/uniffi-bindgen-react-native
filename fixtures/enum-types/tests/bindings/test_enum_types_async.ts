/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-enum-types -- async_wasm

import { asyncTest } from "@/asserts";
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
  OptionalFields,
  OptionalFields_Tags,
  identityOptionalFields,
} from "@/generated/enum_types";

(async () => {
  await asyncTest("Enum discriminant", async (t) => {
    t.assertEqual(Animal.Dog, 0);
    t.assertEqual(Animal.Cat, 1);
    t.assertEqual(await getAnimal(undefined), Animal.Dog);
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
    t.end();
  });

  await asyncTest("Roundtripping enums with values", async (t) => {
    async function assertEqual(
      left: AnimalAssociatedType,
      right: AnimalAssociatedType,
    ): Promise<void> {
      t.assertEqual(left.tag, right.tag);
      switch (left.tag) {
        case AnimalAssociatedType_Tags.Cat:
          return;
        case AnimalAssociatedType_Tags.Dog:
          if (AnimalAssociatedType.Dog.instanceOf(right)) {
            t.assertEqual(
              await left.inner[0].record(),
              await right.inner[0].record(),
            );
          } else {
            t.fail(`${right} is not a Dog`);
          }
      }
    }
    const values = [
      AnimalAssociatedType.Cat.new(),
      AnimalAssociatedType.Dog.new(await AnimalObject.create(1)),
      AnimalAssociatedType.Dog.new(await AnimalObject.create(2)),
    ];

    for (const v of values) {
      t.assertTrue(AnimalAssociatedType.instanceOf(v as any));
      await assertEqual(v, await identityEnumWithAssociatedType(v));
    }
    t.end();
  });

  await asyncTest("Roundtripping enums with name values", async (t) => {
    async function assertEqual(
      left: AnimalNamedAssociatedType,
      right: AnimalNamedAssociatedType,
    ): Promise<void> {
      t.assertEqual(left.tag, right.tag);
      switch (left.tag) {
        case AnimalNamedAssociatedType_Tags.Cat:
          return;
        case AnimalNamedAssociatedType_Tags.Dog:
          if (AnimalNamedAssociatedType.Dog.instanceOf(right)) {
            t.assertEqual(
              await left.inner.value.record(),
              await right.inner.value.record(),
            );
          } else {
            t.fail(`${right} is not a Dog`);
          }
      }
    }
    const values = [
      AnimalNamedAssociatedType.Cat.new(),
      AnimalNamedAssociatedType.Dog.new({
        value: await AnimalObject.create(1),
      }),
      AnimalNamedAssociatedType.Dog.new({
        value: await AnimalObject.create(2),
      }),
    ];

    for (const v of values) {
      t.assertTrue(AnimalNamedAssociatedType.instanceOf(v as any));
      await assertEqual(v, await identityEnumWithNamedAssociatedType(v));
    }
    t.end();
  });

  await asyncTest(
    "Variant naming cam collide with existing types",
    async (t) => {
      // Variants compare by field: assertEqual on variants compares toString(), which is vacuous here.
      {
        const record = AnimalRecord.create({ value: 5 });
        const variant1 = CollidingVariants.AnimalRecord.new(record);
        const variant2 = new CollidingVariants.AnimalRecord(record);

        t.assertEqual(variant1.tag, CollidingVariants_Tags.AnimalRecord);
        t.assertEqual(variant2.tag, CollidingVariants_Tags.AnimalRecord);
        t.assertEqual(variant1.inner[0].value, variant2.inner[0].value);
        const rt = await identityCollidingVariants(variant1);
        if (CollidingVariants.AnimalRecord.instanceOf(rt)) {
          t.assertEqual(rt.inner[0].value, variant2.inner[0].value);
        } else {
          t.fail("expected AnimalRecord");
        }

        t.assertTrue(CollidingVariants.instanceOf(variant1));
        t.assertTrue(CollidingVariants.AnimalRecord.instanceOf(variant1));
      }
      {
        const obj = await AnimalObject.create(1);
        const variant1 = CollidingVariants.AnimalObject.new(obj);
        const variant2 = new CollidingVariants.AnimalObject(obj);

        t.assertEqual(variant1.tag, CollidingVariants_Tags.AnimalObject);
        t.assertEqual(variant2.tag, CollidingVariants_Tags.AnimalObject);
        t.assertEqual(
          await variant1.inner[0].record(),
          await variant2.inner[0].record(),
        );
        const rt = await identityCollidingVariants(variant1);
        if (CollidingVariants.AnimalObject.instanceOf(rt)) {
          t.assertEqual(
            await rt.inner[0].record(),
            await variant2.inner[0].record(),
          );
        } else {
          t.fail("expected AnimalObject");
        }

        t.assertTrue(CollidingVariants.instanceOf(variant1));
        t.assertTrue(CollidingVariants.AnimalObject.instanceOf(variant1));
      }

      {
        const obj = await AnimalObject.create(1);
        const variant1 = CollidingVariants.AnimalObjectInterface.new(obj);
        const variant2 = new CollidingVariants.AnimalObjectInterface(obj);

        t.assertEqual(
          variant1.tag,
          CollidingVariants_Tags.AnimalObjectInterface,
        );
        t.assertEqual(
          variant2.tag,
          CollidingVariants_Tags.AnimalObjectInterface,
        );
        t.assertEqual(
          await variant1.inner[0].record(),
          await variant2.inner[0].record(),
        );
        const rt = await identityCollidingVariants(variant1);
        if (CollidingVariants.AnimalObjectInterface.instanceOf(rt)) {
          t.assertEqual(
            await rt.inner[0].record(),
            await variant2.inner[0].record(),
          );
        } else {
          t.fail("expected AnimalObjectInterface");
        }

        t.assertTrue(CollidingVariants.instanceOf(variant1));
        t.assertTrue(
          CollidingVariants.AnimalObjectInterface.instanceOf(variant1),
        );
      }
      {
        const animal = Animal.Dog;
        const variant1 = CollidingVariants.Animal.new(animal);
        const variant2 = new CollidingVariants.Animal(animal);

        t.assertEqual(variant1.tag, CollidingVariants_Tags.Animal);
        t.assertEqual(variant2.tag, CollidingVariants_Tags.Animal);
        t.assertEqual(variant1.inner[0], variant2.inner[0]);
        const rt = await identityCollidingVariants(variant1);
        if (CollidingVariants.Animal.instanceOf(rt)) {
          t.assertEqual(rt.inner[0], variant2.inner[0]);
        } else {
          t.fail("expected Animal");
        }

        t.assertTrue(CollidingVariants.instanceOf(variant1));
        t.assertTrue(CollidingVariants.Animal.instanceOf(variant1));
      }

      {
        const variant1 = CollidingVariants.CollidingVariants.new();
        const variant2 = new CollidingVariants.CollidingVariants();

        t.assertEqual(variant1.tag, CollidingVariants_Tags.CollidingVariants);
        t.assertEqual(variant2.tag, CollidingVariants_Tags.CollidingVariants);
        const rt = await identityCollidingVariants(variant1);
        t.assertEqual(rt.tag, CollidingVariants_Tags.CollidingVariants);

        t.assertTrue(CollidingVariants.instanceOf(variant1));
        t.assertTrue(CollidingVariants.CollidingVariants.instanceOf(variant1));
      }
      t.end();
    },
  );

  await asyncTest(
    "Variant with Option fields accepts omitted keys and undefined",
    async (t) => {
      // Omitting optional keys type-checks only while the keys stay `?:`.
      const v1 = OptionalFields.Named.new({ required: "hello" });
      t.assertEqual(v1.tag, OptionalFields_Tags.Named);
      if (OptionalFields.Named.instanceOf(v1)) {
        t.assertEqual(v1.inner.required, "hello");
        t.assertEqual(v1.inner.maybeString, undefined);
        t.assertEqual(v1.inner.maybeRecord, undefined);
      } else {
        t.fail("expected Named");
      }

      // Explicit `undefined` is also accepted.
      const v2 = new OptionalFields.Named({
        required: "world",
        maybeString: undefined,
        maybeRecord: undefined,
      });
      t.assertEqual(
        (await identityOptionalFields(v2)).tag,
        OptionalFields_Tags.Named,
      );

      // Supplying values roundtrips.
      const v3 = OptionalFields.Named.new({
        required: "r",
        maybeString: "s",
        maybeRecord: AnimalRecord.create({ value: 7 }),
      });
      const roundTripped = await identityOptionalFields(v3);
      if (OptionalFields.Named.instanceOf(roundTripped)) {
        t.assertEqual(roundTripped.inner.required, "r");
        t.assertEqual(roundTripped.inner.maybeString, "s");
        t.assertEqual(roundTripped.inner.maybeRecord?.value, 7);
      } else {
        t.fail("expected Named");
      }
      t.end();
    },
  );
})();

// An example of how to use enums with values; nothing calls it.
//
// In each of the variants, we switch match on the `variant.tag`. Typescript then
// infers the type of `variant.inner`.
//
// In this particular example, each of the variants has a tuple value, of length 1.
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

// An example of how to use enums with values; nothing calls it.
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

// An example of how to use enums with values; nothing calls it.
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
