/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// fixture=coverall
// cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- bindings ./fixtures/${fixture}/src/${fixture}.udl --cpp-dir ./fixtures/${fixture}/generated --ts-dir ./fixtures/${fixture}/generated
// cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}

import {
  Coveralls,
  createNoneDict,
  createSomeDict,
  getNumAlive,
} from "../../generated/coverall";
import { assertEqual, assertFalse, assertTrue, test, xtest } from "@/asserts";
import { console } from "@/hermes";

// floats should be "close enough".
const almostEquals = (this_: number, that: number): boolean =>
  Math.abs(this_ - that) < 0.000001;

test("test create_some_dict() with default values", () => {
  const d = createSomeDict();
  assertEqual(d.text, "text");
  assertEqual(d.maybeText, "maybe_text");
  // Hermes doesn't support string --> ArrayBuffer
  // assertEqual(d.someBytes.contentEquals("some_bytes".toByteArray(Charsets.UTF_8)))
  // assertEqual(d.maybeSomeBytes.contentEquals("maybe_some_bytes".toByteArray(Charsets.UTF_8)))
  assertTrue(d.aBool);
  assertEqual(d.maybeABool, false);
  assertEqual(d.unsigned8, 1);
  assertEqual(d.maybeUnsigned8, 2);
  assertEqual(d.unsigned16, 3);
  assertEqual(d.maybeUnsigned16, 4);
  // Test failing: observed: 18446744073709551616
  // assertEqual(d.unsigned64, BigInt("18446744073709551615"))
  assertEqual(d.maybeUnsigned64, BigInt("0"));
  assertEqual(d.signed8, 8);
  assertEqual(d.maybeSigned8, 0);
  // Test failing: observed: 9223372036854775808
  // assertEqual(d.signed64, BigInt("9223372036854775807"))
  assertEqual(d.maybeSigned64, BigInt("0"));

  assertEqual(d.float32, 1.2345, undefined, almostEquals);
  assertEqual(d.maybeFloat32!, 22.0 / 7.0, undefined, almostEquals);
  assertEqual(d.float64, 0, undefined, almostEquals);
  assertEqual(d.maybeFloat64!, 1.0, undefined, almostEquals);

  assertEqual(d.coveralls!.getName(), "some_dict");
});

test("test create_none_dict() with default values", () => {
  const d = createNoneDict();
  assertEqual(d.text, "text");
  assertEqual(d.maybeText, undefined);
  // Hermes doesn't support string --> ArrayBuffer
  // assertEqual(d.someBytes.contentEquals("some_bytes".toByteArray(Charsets.UTF_8)))
  // assertEqual(d.maybeSomeBytes.contentEquals("maybe_some_bytes".toByteArray(Charsets.UTF_8)))
  assertTrue(d.aBool);
  assertEqual(d.maybeABool, undefined);
  assertEqual(d.unsigned8, 1);
  assertEqual(d.maybeUnsigned8, undefined);
  assertEqual(d.unsigned16, 3);
  assertEqual(d.maybeUnsigned16, undefined);
  // Test failing: observed: 18446744073709551616
  // assertEqual(d.unsigned64, BigInt("18446744073709551615"))
  assertEqual(d.maybeUnsigned64, undefined);
  assertEqual(d.signed8, 8);
  assertEqual(d.maybeSigned8, undefined);
  // Test failing: observed: 9223372036854775808
  // assertEqual(d.signed64, BigInt("9223372036854775807"))
  assertEqual(d.maybeSigned64, undefined);

  assertEqual(d.float32, 1.2345, undefined, almostEquals);
  assertEqual(d.maybeFloat32, undefined);
  assertEqual(d.float64, 0, undefined, almostEquals);
  assertEqual(d.maybeFloat64, undefined);

  assertEqual(d.coveralls, undefined);
});

test("Given 1000 objects, when they go out of scope, then they are dropped by rust", () => {
  // The GC test; we should have 1000 alive by the end of the loop.
  //
  // Later on, nearer the end of the script, we'll test again, when the cleaner
  // has had time to clean up.
  //
  // The number alive then should be zero.
  const makeCoveralls = (n: number): void => {
    for (let i = 0; i < n; i++) {
      const c = new Coveralls(`GC testing ${i}`);
    }
  };

  const initial = getNumAlive();

  // First we make 1000 objects, and wait for the rest of the test to run. If it has, then
  // the garbage objects have been collected, and the Rust counter parts have been dropped.
  makeCoveralls(1000);

  assertEqual(getNumAlive(), initial);
});
