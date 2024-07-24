/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// fixture=coverall
// cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- bindings ./fixtures/${fixture}/src/${fixture}.udl --cpp-dir ./fixtures/${fixture}/generated --ts-dir ./fixtures/${fixture}/generated
// cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}

import {
  CoverallException,
  ComplexException,
  Coveralls,
  createNoneDict,
  createSomeDict,
  getNumAlive,
  RootException,
  throwRootError,
  getRootError,
  OtherError,
  getComplexError,
  getErrorDict,
} from "../../generated/coverall";
import { test } from "@/asserts";
import { console } from "@/hermes";

// floats should be "close enough".
const almostEquals = (this_: number, that: number): boolean =>
  Math.abs(this_ - that) < 0.000001;

test("test create_some_dict() with default values", (t) => {
  const d = createSomeDict();
  t.assertEqual(d.text, "text");
  t.assertEqual(d.maybeText, "maybe_text");
  // Hermes doesn't support string --> ArrayBuffer
  // t.assertEqual(d.someBytes.contentEquals("some_bytes".toByteArray(Charsets.UTF_8)))
  // t.assertEqual(d.maybeSomeBytes.contentEquals("maybe_some_bytes".toByteArray(Charsets.UTF_8)))
  t.assertTrue(d.aBool);
  t.assertEqual(d.maybeABool, false);
  t.assertEqual(d.unsigned8, 1);
  t.assertEqual(d.maybeUnsigned8, 2);
  t.assertEqual(d.unsigned16, 3);
  t.assertEqual(d.maybeUnsigned16, 4);
  t.assertEqual(d.unsigned64, BigInt("0x10000000000000000"));
  t.assertEqual(d.maybeUnsigned64, BigInt("0"));
  t.assertEqual(d.signed8, 8);
  t.assertEqual(d.maybeSigned8, 0);
  t.assertEqual(d.signed64, BigInt("0x8000000000000000"));
  t.assertEqual(d.maybeSigned64, BigInt("0"));

  t.assertEqual(d.float32, 1.2345, undefined, almostEquals);
  t.assertEqual(d.maybeFloat32!, 22.0 / 7.0, undefined, almostEquals);
  t.assertEqual(d.float64, 0, undefined, almostEquals);
  t.assertEqual(d.maybeFloat64!, 1.0, undefined, almostEquals);

  t.assertEqual(d.coveralls!.getName(), "some_dict");
});

test("test create_none_dict() with default values", (t) => {
  const d = createNoneDict();
  t.assertEqual(d.text, "text");
  t.assertEqual(d.maybeText, undefined);
  // Hermes doesn't support string --> ArrayBuffer
  // t.assertEqual(d.someBytes.contentEquals("some_bytes".toByteArray(Charsets.UTF_8)))
  // t.assertEqual(d.maybeSomeBytes.contentEquals("maybe_some_bytes".toByteArray(Charsets.UTF_8)))
  t.assertTrue(d.aBool);
  t.assertEqual(d.maybeABool, undefined);
  t.assertEqual(d.unsigned8, 1);
  t.assertEqual(d.maybeUnsigned8, undefined);
  t.assertEqual(d.unsigned16, 3);
  t.assertEqual(d.maybeUnsigned16, undefined);
  t.assertEqual(d.unsigned64, BigInt("0x10000000000000000"));
  t.assertEqual(d.maybeUnsigned64, undefined);
  t.assertEqual(d.signed8, 8);
  t.assertEqual(d.maybeSigned8, undefined);
  t.assertEqual(d.signed64, BigInt("0x8000000000000000"));
  t.assertEqual(d.maybeSigned64, undefined);

  t.assertEqual(d.float32, 1.2345, undefined, almostEquals);
  t.assertEqual(d.maybeFloat32, undefined);
  t.assertEqual(d.float64, 0, undefined, almostEquals);
  t.assertEqual(d.maybeFloat64, undefined);

  t.assertEqual(d.coveralls, undefined);
});

test("Given 1000 objects, when they go out of scope, then they are dropped by rust", (t) => {
  // The GC test; we should have 1000 alive by the end of the loop.
  //
  // Later on, nearer the end of the script, we'll test again, when the cleaner
  // has had time to clean up.
  //
  // The number alive then should be zero.
  const makeCoveralls = (n: number): void => {
    for (let i = 0; i < n; i++) {
      const c = new Coveralls(`GC testing ${i}`);
      // The test should not have this destroy method here: we're explictly
      // tyring to test that the JS GC calls the detructor.
      // Currently it does not.
      c.uniffiDestroy();
    }
  };

  const initial = getNumAlive();

  // First we make 1000 objects, and wait for the rest of the test to run. If it has, then
  // the garbage objects have been collected, and the Rust counter parts have been dropped.
  makeCoveralls(1000);

  t.assertEqual(getNumAlive(), initial);
});

test("Simple Errors", (t) => {
  const coveralls = new Coveralls("Testing simple errors");
  // should not throw an error.
  coveralls.maybeThrow(false);

  // Do the long hand way of catching errors.
  try {
    coveralls.maybeThrow(false);
    t.fail("An error should've been thrown");
  } catch (e: any) {
    // OK
  }
  // Now the short hand.
  t.assertThrows(CoverallException.TooManyHoles.instanceOf, () =>
    coveralls.maybeThrow(true),
  );
  t.assertThrows(CoverallException.TooManyHoles.instanceOf, () =>
    coveralls.maybeThrowInto(true),
  );
  coveralls.uniffiDestroy();
});

test("Complex errors", (t) => {
  const coveralls = new Coveralls("Testing complex errors");
  // No errors to throw with 0.
  t.assertTrue(coveralls.maybeThrowComplex(0));

  t.assertThrows(ComplexException.OsError.instanceOf, () => {
    coveralls.maybeThrowComplex(1);
  });
  coveralls.uniffiDestroy();
});

test("Error Values", (t) => {
  const coveralls = new Coveralls("Testing error values");
  t.assertThrows(RootException.Complex.instanceOf, () => {
    throwRootError();
  });
  t.assertThrows(
    (e) => ComplexException.instanceOf(e.error),
    () => {
      throwRootError();
    },
  );

  const e = getRootError();
  t.assertTrue(RootException.Other.instanceOf(e));
  t.assertEqual(e.error, OtherError.UNEXPECTED);

  const ce = getComplexError(undefined);
  t.assertTrue(ComplexException.PermissionDenied.instanceOf(ce));
  t.assertNull(getErrorDict(undefined).complexError);

  coveralls.uniffiDestroy();
});
