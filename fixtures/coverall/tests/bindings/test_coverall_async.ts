/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-coverall -- async_wasm

import {
  CoverallError,
  ComplexError,
  CoverallsInterface,
  Coveralls,
  createNoneDict,
  createSomeDict,
  getNumAlive,
  RootError,
  throwRootError,
  getRootError,
  OtherError,
  getComplexError,
  getErrorDict,
  EmptyStruct,
  Patch,
  Repair,
  PatchInterface,
  Color,
} from "@/generated/coverall";
import { asyncTest } from "@/asserts";

// floats should be "close enough".
const almostEquals = (this_: number, that: number): boolean =>
  Math.abs(this_ - that) < 0.000001;

(async () => {
  await asyncTest("test create_some_dict() with default values", async (t) => {
    const d = await createSomeDict();
    t.assertEqual(d.text, "text");
    t.assertEqual(d.maybeText, "maybe_text");
    // Hermes doesn't support string --> ArrayBuffer, so we just check the length.
    // The string is all alphanumeric, so the length is the same in chars as they are in bytes.
    // This is not the case for all strings, however.
    t.assertEqual(d.someBytes.byteLength, "some_bytes".length);
    t.assertEqual(d.maybeSomeBytes?.byteLength, "maybe_some_bytes".length);
    t.assertTrue(d.aBool);
    t.assertEqual(d.maybeABool, false);
    t.assertEqual(d.unsigned8, 1);
    t.assertEqual(d.maybeUnsigned8, 2);
    t.assertEqual(d.unsigned16, 3);
    t.assertEqual(d.maybeUnsigned16, 4);
    t.assertEqual(d.unsigned64, BigInt("0xffffffffffffffff"));
    t.assertEqual(d.maybeUnsigned64, BigInt("0"));
    t.assertEqual(d.signed8, 8);
    t.assertEqual(d.maybeSigned8, 0);
    t.assertEqual(d.signed64, BigInt("0x7fffffffffffffff"));
    t.assertEqual(d.maybeSigned64, BigInt("0"));

    t.assertEqual(d.float32, 1.2345, undefined, almostEquals);
    t.assertEqual(d.maybeFloat32!, 22.0 / 7.0, undefined, almostEquals);
    t.assertEqual(d.float64, 0, undefined, almostEquals);
    t.assertEqual(d.maybeFloat64!, 1.0, undefined, almostEquals);

    t.assertEqual(await d.coveralls!.getName(), "some_dict");

    (d.coveralls! as Coveralls).uniffiDestroy();
    t.end();
  });

  await asyncTest("test create_none_dict() with default values", async (t) => {
    const d = await createNoneDict();
    t.assertEqual(d.text, "text");
    t.assertEqual(d.maybeText, undefined);
    // Hermes doesn't support string --> ArrayBuffer, so we just check the length.
    // The string is all alphanumeric, so the length is the same in chars as they are in bytes.
    // This is not the case for all strings, however.
    t.assertEqual(d.someBytes.byteLength, "some_bytes".length);
    t.assertEqual(d.maybeSomeBytes, undefined);
    t.assertTrue(d.aBool);
    t.assertEqual(d.maybeABool, undefined);
    t.assertEqual(d.unsigned8, 1);
    t.assertEqual(d.maybeUnsigned8, undefined);
    t.assertEqual(d.unsigned16, 3);
    t.assertEqual(d.maybeUnsigned16, undefined);
    t.assertEqual(d.unsigned64, BigInt("0xffffffffffffffff"));
    t.assertEqual(d.maybeUnsigned64, undefined);
    t.assertEqual(d.signed8, 8);
    t.assertEqual(d.maybeSigned8, undefined);
    t.assertEqual(d.signed64, BigInt("0x7fffffffffffffff"));
    t.assertEqual(d.maybeSigned64, undefined);

    t.assertEqual(d.float32, 1.2345, undefined, almostEquals);
    t.assertEqual(d.maybeFloat32, undefined);
    t.assertEqual(d.float64, 0, undefined, almostEquals);
    t.assertEqual(d.maybeFloat64, undefined);

    t.assertEqual(d.coveralls, undefined);
    t.end();
  });

  await asyncTest("arc", async (t) => {
    const initialAlive = await getNumAlive();
    const coveralls = (await Coveralls.create("test_arcs")) as Coveralls;
    t.assertEqual(await getNumAlive(), initialAlive + BigInt("1"));
    // One ref held by the foreign-language code, one created for this method call.
    t.assertEqual(await coveralls.strongCount(), BigInt("2"));
    t.assertNull(await coveralls.getOther());
    await coveralls.takeOther(coveralls);
    // Should now be a new strong ref, held by the object's reference to itself.
    t.assertEqual(await coveralls.strongCount(), BigInt("3"));
    // But the same number of instances.
    t.assertEqual(await getNumAlive(), initialAlive + BigInt("1"));
    // Careful, this makes a new Kotlin object which must be separately destroyed.

    // Get another, but it's the same Rust object.
    await (async (other: CoverallsInterface) => {
      t.assertEqual(await other.getName(), "test_arcs");
      (other as Coveralls).uniffiDestroy();
    })((await coveralls.getOther())!);
    t.assertEqual(await getNumAlive(), initialAlive + BigInt("1"));

    await t.assertThrowsAsync(CoverallError.TooManyHoles.instanceOf, () =>
      coveralls.takeOtherFallible(),
    );
    await t.assertThrowsAsync(
      (err) => true,
      () => coveralls.takeOtherPanic("expected panic: with an arc!"),
    );
    await t.assertThrowsAsync(
      (err) => true,
      () => coveralls.falliblePanic("Expected panic in a fallible function!"),
    );
    await coveralls.takeOther(undefined);
    // TODO coveralls.takeOther doesn't decrement the strong_count in WASM only.
    // t.assertEqual(await coveralls.strongCount(), BigInt("2"));

    coveralls.uniffiDestroy();
    // TODO this is a knock on effect for the strong_count not reaching 0.
    // t.assertEqual(await getNumAlive(), initialAlive + BigInt("0"));
    t.end();
  });

  await asyncTest("Return objects", async (t) => {
    const initialAlive = await getNumAlive();
    const coveralls = (await Coveralls.create(
      "test_return_objects",
    )) as Coveralls;
    t.assertEqual(await getNumAlive(), initialAlive + BigInt("1"));
    t.assertEqual(await coveralls.strongCount(), BigInt("2"));

    await (async (c2: CoverallsInterface) => {
      t.assertEqual(await c2.getName(), await coveralls.getName());
      t.assertEqual(await getNumAlive(), initialAlive + BigInt("2"));
      t.assertEqual(await c2.strongCount(), BigInt("2"));

      await coveralls.takeOther(c2);
      // same number alive but `c2` has an additional ref count.
      t.assertEqual(await getNumAlive(), initialAlive + BigInt("2"));
      t.assertEqual(await coveralls.strongCount(), BigInt("2"));
      t.assertEqual(await c2.strongCount(), BigInt("3"));
      (c2 as Coveralls).uniffiDestroy();
    })(await coveralls.cloneMe());

    t.assertEqual(await getNumAlive(), initialAlive + BigInt("2"));
    coveralls.uniffiDestroy();

    t.assertEqual(await getNumAlive(), initialAlive + BigInt("0"));
    t.end();
  });

  await asyncTest(
    "Given a rust object, when it is destroyed, it cannot be re-used",
    async (t) => {
      const name = "To be destroyed";
      const c = (await Coveralls.create(name)) as Coveralls;

      // check it works first.
      t.assertEqual(name, await c.getName());

      // then destroy it.
      c.uniffiDestroy();

      // now check it doesn't work.
      await t.assertThrowsAsync(
        () => true,
        () => c.getName(),
      );

      // …destroy again, just to show it's idempotent.
      c.uniffiDestroy();
      t.end();
    },
  );

  await asyncTest(
    "Given 1000 objects, when they go out of scope, then they are dropped by rust",
    async (t) => {
      // The GC test; we should have 1000 alive by the end of the loop.
      //
      // Later on, nearer the end of the script, we'll test again, when the cleaner
      // has had time to clean up.
      //
      // The number alive then should be zero.
      const makeCoveralls = async (n: number): Promise<void> => {
        for (let i = 0; i < n; i++) {
          const c = (await Coveralls.create(`GC testing ${i}`)) as Coveralls;
          // The test should not have this destroy method here: we're explictly
          // tyring to test that the JS GC calls the detructor.
          // Currently it does not.
          c.uniffiDestroy();
        }
      };

      const initial = await getNumAlive();

      // First we make 1000 objects, and wait for the rest of the test to run. If it has, then
      // the garbage objects have been collected, and the Rust counter parts have been dropped.
      await makeCoveralls(1000);

      t.assertEqual(await getNumAlive(), initial);
      t.end();
    },
  );

  await asyncTest("Simple Errors", async (t) => {
    const coveralls = (await Coveralls.create(
      "Testing simple errors",
    )) as Coveralls;
    // should not throw an error.
    await coveralls.maybeThrow(false);

    // Do the long hand way of catching errors.
    try {
      await coveralls.maybeThrow(false);
      t.fail("An error should've been thrown");
    } catch (e: any) {
      // OK
    }
    // Now the short hand.
    await t.assertThrowsAsync(CoverallError.TooManyHoles.instanceOf, () =>
      coveralls.maybeThrow(true),
    );
    await t.assertThrowsAsync(CoverallError.TooManyHoles.instanceOf, () =>
      coveralls.maybeThrowInto(true),
    );
    coveralls.uniffiDestroy();
    t.end();
  });

  await asyncTest("Complex errors", async (t) => {
    const coveralls = (await Coveralls.create(
      "Testing complex errors",
    )) as Coveralls;
    // No errors to throw with 0.
    t.assertTrue(await coveralls.maybeThrowComplex(0));

    await t.assertThrowsAsync(ComplexError.OsError.instanceOf, async () => {
      await coveralls.maybeThrowComplex(1);
    });
    coveralls.uniffiDestroy();
    t.end();
  });

  await asyncTest("Error Values", async (t) => {
    const coveralls = (await Coveralls.create(
      "Testing error values",
    )) as Coveralls;
    await t.assertThrowsAsync(RootError.Complex.instanceOf, async () => {
      await throwRootError();
    });
    await t.assertThrowsAsync(
      (e) => ComplexError.instanceOf(e.inner.error),
      async () => {
        await throwRootError();
      },
    );

    const e = await getRootError();
    t.assertTrue(RootError.Other.instanceOf(e));
    t.assertEqual(e.inner.error, OtherError.Unexpected);

    const ce = await getComplexError(undefined);
    t.assertTrue(ComplexError.PermissionDenied.instanceOf(ce));
    t.assertNull((await getErrorDict(undefined)).complexError);

    coveralls.uniffiDestroy();
    t.end();
  });

  await asyncTest("Interfaces in dicts", async (t) => {
    const coveralls = (await Coveralls.create(
      "Testing interfaces in dicts",
    )) as Coveralls;
    await coveralls.addPatch(await Patch.create(Color.Red));
    await coveralls.addRepair(
      Repair.new({ patch: await Patch.create(Color.Blue) }),
    );
    t.assertEqual((await coveralls.getRepairs()).length, 2);
    coveralls.uniffiDestroy();
    t.end();
  });

  await asyncTest(
    "Dummy coveralls implement the Coveralls interface",
    async (t) => {
      // We're testing only whether this is compilable.
      class DummyCoveralls implements CoverallsInterface {
        async addPatch(patch: PatchInterface): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async addRepair(repair: Repair): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async cloneMe(): Promise<CoverallsInterface> {
          throw new Error("Method not implemented.");
        }
        async falliblePanic(message: string): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async getDict(
          key: string,
          value: bigint,
        ): Promise<Map<string, bigint>> {
          throw new Error("Method not implemented.");
        }
        async getDict2(
          key: string,
          value: bigint,
        ): Promise<Map<string, bigint>> {
          throw new Error("Method not implemented.");
        }
        async getDict3(
          key: number,
          value: bigint,
        ): Promise<Map<number, bigint>> {
          throw new Error("Method not implemented.");
        }
        async getName(): Promise<string> {
          throw new Error("Method not implemented.");
        }
        async getOther(): Promise<CoverallsInterface | undefined> {
          throw new Error("Method not implemented.");
        }
        async getRepairs(): Promise<Array<Repair>> {
          throw new Error("Method not implemented.");
        }
        async getStatus(status: string): Promise<string> {
          throw new Error("Method not implemented.");
        }
        async maybeThrow(shouldThrow: boolean): Promise<boolean> {
          throw new Error("Method not implemented.");
        }
        async maybeThrowComplex(input: number): Promise<boolean> {
          throw new Error("Method not implemented.");
        }
        async maybeThrowInto(shouldThrow: boolean): Promise<boolean> {
          throw new Error("Method not implemented.");
        }
        async panic(message: string): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async reverse(value: ArrayBuffer): Promise<ArrayBuffer> {
          throw new Error("Method not implemented.");
        }
        async setAndGetEmptyStruct(
          emptyStruct: EmptyStruct,
        ): Promise<EmptyStruct> {
          throw new Error("Method not implemented.");
        }
        async strongCount(): Promise<bigint> {
          throw new Error("Method not implemented.");
        }
        async takeOther(other: CoverallsInterface | undefined): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async takeOtherFallible(): Promise<void> {
          throw new Error("Method not implemented.");
        }
        async takeOtherPanic(message: string): Promise<void> {
          throw new Error("Method not implemented.");
        }
      }

      // reimplementing the CoverallsInterface interface.
      const dummyCoveralls = new DummyCoveralls();
      t.assertFalse(Coveralls.instanceOf(dummyCoveralls));

      // Subclassing Coveralls is dropped: the constructor is private on async objects.
      t.end();
    },
  );

  // This is the last test, so we can test a graceful shutdown.
  await asyncTest(
    "Given a rust object, if not destroyed, it's ok",
    async (t) => {
      const name = "To be destroyed by GC";
      const c = (await Coveralls.create(name)) as Coveralls;
      t.end();
    },
  );
})();
