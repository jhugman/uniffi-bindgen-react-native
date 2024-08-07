/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import myModule, {
  ComplexError,
  ForeignGetters,
  SimpleError,
  RustStringifier,
  RustGetters,
  StoredForeignStringifier,
} from "../../generated/callbacks";
import { test } from "@/asserts";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

const BAD_ARGUMENT = "bad-argument";
const UNEXPECTED_ERROR = "unexpected-error";
const SOMETHING_FAILED = "something-failed";
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
      throw new ComplexError.ReallyBadArgument({ code: 20 });
    }
    if (v == UNEXPECTED_ERROR) {
      throw Error(SOMETHING_FAILED);
    }
    return arg2 ? v?.toUpperCase() : v;
  }
  getList(v: number[], arg2: boolean): number[] {
    return arg2 ? v : [];
  }
  getNothing(v: string): void {
    if (v == BAD_ARGUMENT) {
      throw new SimpleError.BadArgument(BAD_ARGUMENT);
    }
    if (v == UNEXPECTED_ERROR) {
      throw Error(SOMETHING_FAILED);
    }
  }
}

test("Boolean values passed between callback interfaces", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.boolean) {
    const expected = callbackInterface.getBool(v, flag);
    const observed = rg.getBool(callbackInterface, v, flag);
    t.assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("List values passed between callback interfaces", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.string) {
    const expected = callbackInterface.getString(v, flag);
    const observed = rg.getString(callbackInterface, v, flag);
    t.assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("String values passed between callback interfaces", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = true;
  for (const v of inputData.listInt) {
    const expected = callbackInterface.getList(v, flag);
    const observed = rg.getList(callbackInterface, v, flag);
    t.assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("Optional callbacks serialized correctly", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  t.assertEqual(
    rg.getStringOptionalCallback(callbackInterface, "TestString", false),
    "TestString",
  );
  t.assertNull(rg.getStringOptionalCallback(undefined, "TestString", false));
  rg.uniffiDestroy();
});

test("Flat errors are propagated correctly", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  t.assertThrows(SimpleError.BadArgument.instanceOf, () =>
    rg.getNothing(callbackInterface, BAD_ARGUMENT),
  );
  t.assertThrows(SimpleError.UnexpectedError.instanceOf, () =>
    rg.getNothing(callbackInterface, UNEXPECTED_ERROR),
  );
  rg.uniffiDestroy();
});

test("Non-flat errors are propagated correctly", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  t.assertThrows(
    (err) => {
      const isError = ComplexError.ReallyBadArgument.instanceOf(err);
      if (isError) {
        // set in TypesSriptGetters.getOption
        t.assertEqual(err.inner.code, 20);
      }
      return isError;
    },
    () => rg.getOption(callbackInterface, BAD_ARGUMENT, true),
  );
  t.assertThrows(
    (err) => {
      const isError = ComplexError.UnexpectedErrorWithReason.instanceOf(err);
      if (isError) {
        t.assertEqual(err.inner.reason, `Error: ${SOMETHING_FAILED}`);
      }
      return isError;
    },
    () => rg.getOption(callbackInterface, UNEXPECTED_ERROR, true),
  );
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

test("A callback passed into the constructor", (t) => {
  const stringifierCallback = new StoredTypeScriptStringifier();
  const rustStringifier = new RustStringifier(stringifierCallback);

  const expected = stringifierCallback.fromSimpleType(42);
  const observed = rustStringifier.fromSimpleType(42);

  t.assertEqual(observed, expected);

  rustStringifier.uniffiDestroy();
});

test("A callback passed into the constructor is not destroyed", (t) => {
  const stringifierCallback = new StoredTypeScriptStringifier();
  const rustStringifier = new RustStringifier(stringifierCallback);

  // The destructor calls into the Rust, which calls back into Javascript.
  // If the jsi::Runtime has been destroyed already, then this will cause a
  // crash at the end of the run. This should be prevented.
});
