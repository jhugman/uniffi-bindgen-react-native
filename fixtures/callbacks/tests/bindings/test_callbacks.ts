/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  ComplexException,
  ForeignGetters,
  SimpleException,
  RustStringifier,
  RustGetters,
  StoredForeignStringifier,
} from "../../generated/callbacks";
import { assertEqual, assertNull, assertThrows, fail, test } from "@/asserts";

const BAD_ARGUMENT = "bad-argument";
const UNEXPECTED_ERROR = "unexpected-error";
const inputData = {
  boolean: [true, false],
  listInt: [
    [0, 1],
    [0, 2, 4],
  ],
  string: [
    "",
    "abc",
    "null\u0000byte",
    "été",
    "ښي لاس ته لوستلو لوستل",
    "😻emoji 👨‍👧‍👦multi-emoji, 🇨🇭a flag, a canal, panama",
  ],
};

class TypeScriptGetters implements ForeignGetters {
  getBool(v: boolean, argumentTwo: boolean): boolean {
    return v !== argumentTwo;
  }
  getString(v: string, arg2: boolean): string {
    this.getNothing(v);
    return arg2 ? "1234567890123" : v;
  }
  getOption(v: string | undefined, arg2: boolean): string | undefined {
    if (v == BAD_ARGUMENT) {
      // throw new ComplexException.ReallyBadArgument(20);
      throw new SimpleException.BadArgument(BAD_ARGUMENT);
    }
    if (v == UNEXPECTED_ERROR) {
      throw Error(UNEXPECTED_ERROR);
    }
    return arg2 ? v?.toUpperCase() : v;
  }
  getList(v: number[], arg2: boolean): number[] {
    return arg2 ? v : [];
  }
  getNothing(v: string): void {
    if (v == BAD_ARGUMENT) {
      throw new SimpleException.BadArgument(BAD_ARGUMENT);
    }
    if (v == UNEXPECTED_ERROR) {
      throw new Error(UNEXPECTED_ERROR);
    }
  }
}

test("Boolean values passed between callback interfaces", () => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.boolean) {
    const expected = callbackInterface.getBool(v, flag);
    const observed = rg.getBool(callbackInterface, v, flag);
    assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("List values passed between callback interfaces", () => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.string) {
    const expected = callbackInterface.getString(v, flag);
    const observed = rg.getString(callbackInterface, v, flag);
    assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("String values passed between callback interfaces", () => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.listInt) {
    const expected = callbackInterface.getList(v, flag);
    const observed = rg.getList(callbackInterface, v, flag);
    assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("Optional callbacks serialized correctly", () => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  assertEqual(
    rg.getStringOptionalCallback(callbackInterface, "TestString", false),
    "TestString",
  );
  assertNull(rg.getStringOptionalCallback(undefined, "TestString", false));
  rg.uniffiDestroy();
});

test("Errors are propagated correctly", () => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  assertThrows(SimpleException.BadArgument, () =>
    rg.getNothing(callbackInterface, BAD_ARGUMENT),
  );
  assertThrows(Error, () => rg.getNothing(callbackInterface, UNEXPECTED_ERROR));
  rg.uniffiDestroy();
});

// 2. Pass the callback in as a constructor argument, to be stored on the Object struct.
// This is crucial if we want to configure a system at startup,
// then use it without passing callbacks all the time.
class StoredTypeScriptStringifier implements StoredForeignStringifier {
  fromSimpleType(value: number): string {
    return `typescript: ${value}`;
  }
  // We don't test this, but we're checking that the arg type is included in the minimal
  // list of types used in the UDL.
  // If this doesn't compile, then look at TypeResolver.
  fromComplexType(values: (number | undefined)[] | undefined): string {
    if (values) {
      return `typescript: ${values.join(", ")}`;
    } else {
      return `typescript: none`;
    }
  }
}
test("A callback passed into the constructor", () => {
  const stringifierCallback = new StoredTypeScriptStringifier();
  const rustStringifier = new RustStringifier(stringifierCallback);

  const expected = stringifierCallback.fromSimpleType(42);
  const observed = rustStringifier.fromSimpleType(42);

  assertEqual(observed, expected);
});
