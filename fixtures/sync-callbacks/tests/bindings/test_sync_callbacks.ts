/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-sync-callbacks -- jsi
//   cargo test -p uniffi-fixture-sync-callbacks -- wasm

import syncCallbacks, {
  callDelete,
  ComplexError,
  EventListener,
  EventSource,
  Getters,
  GettersComplexError,
  GettersError,
  makeRustGetters,
  PayloadInterface,
  ForeignGetters,
  KeywordListener,
  RustGetters,
  RustStringifier,
  SimpleError,
  StoredForeignStringifier,
  testGetters,
  testRoundTripThroughForeign,
  testRoundTripThroughRust,
} from "@/generated/sync_callbacks";
import { asyncTest, AsyncAsserts, test } from "@/asserts";
import "@/polyfills";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
syncCallbacks.initialize();

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

test("String values passed between callback interfaces", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = false;
  for (const v of inputData.string) {
    const expected = callbackInterface.getString(v, flag);
    const observed = rg.getString(callbackInterface, v, flag);
    t.assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

test("List values passed between callback interfaces", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  const flag = false;
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

test("Flat errors are propagated correctly, with non-void return", (t) => {
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  t.assertThrows(SimpleError.BadArgument.instanceOf, () =>
    rg.getString(callbackInterface, BAD_ARGUMENT, true),
  );
  t.assertThrows(SimpleError.UnexpectedError.instanceOf, () =>
    rg.getString(callbackInterface, UNEXPECTED_ERROR, true),
  );
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

class EventListenerImpl implements EventListener {
  constructor(
    private t: AsyncAsserts,
    private max: number,
  ) {}
  onEvent(message: string, number: number): void {
    // console.log("--", message, number);
    if (number === this.max - 1) {
      console.log("-- Done!", this.max);
      this.t.end();
    }
  }
}

class KeywordListenerImpl implements KeywordListener {
  delete_(value: string): string {
    console.log("delete_", value);
    return `deleted:${value}`;
  }
}

async function testToMax(max: number, t: AsyncAsserts) {
  const listener = new EventListenerImpl(t, max);
  const source = new EventSource(listener);
  source.start(`Going to ${max}, now at:`, max);
}

test("Reserved C++ keyword callback methods work end-to-end", (t) => {
  const listener = new KeywordListenerImpl();
  t.assertEqual(callDelete(listener, "abc"), "deleted:abc");
});

// A TypeScript implementation of the Getters trait, exercising the foreign implementation path.
class TypeScriptTraitGetters implements Getters {
  getBool(v: boolean, arg2: boolean): boolean {
    return v !== arg2;
  }

  getString(v: string, arg2: boolean): string /*throws*/ {
    if (v === "too-many-holes") {
      throw new GettersError.TooManyHoles("too many holes");
    } else if (v === "unexpected-error") {
      throw new Error("unexpected error");
    }
    return arg2 ? v.toUpperCase() : v;
  }

  getOption(v: string, arg2: boolean): string | undefined /*throws*/ {
    if (v === "os-error") {
      throw new GettersComplexError.OsError({ code: 100, extendedCode: 200 });
    } else if (v === "unknown-error") {
      throw new GettersComplexError.UnknownError();
    } else if (arg2) {
      return v.length > 0 ? v.toUpperCase() : undefined;
    } else {
      return v;
    }
  }

  getList(v: Array<number>, arg2: boolean): Array<number> {
    return arg2 ? v : [];
  }

  getNothing(_v: string): void {
    // do nothing
  }

  roundTripObject(payload: PayloadInterface): PayloadInterface {
    return payload;
  }
}

test("Rust getters: testGetters with a Rust implementation", (t) => {
  const rustGetters = makeRustGetters();
  testGetters(rustGetters);
  // makeRustGetters returns a GettersImpl (Rust object); no explicit destroy available
  // via the Getters interface, so just let it go out of scope.
});

test("TypeScript getters: testGetters with a TypeScript implementation", (t) => {
  const tsGetters = new TypeScriptTraitGetters();
  testGetters(tsGetters);
});

test("Round trip through Rust: TypeScript object is returned unchanged", (t) => {
  const tsGetters = new TypeScriptTraitGetters();
  const returned = testRoundTripThroughRust(tsGetters);
  // The returned object should still work as a Getters implementation.
  t.assertTrue(returned.getBool(true, false));
  t.assertFalse(returned.getBool(true, true));
});

test("Round trip through foreign: Rust calls roundTripObject on TypeScript implementation", (t) => {
  const tsGetters = new TypeScriptTraitGetters();
  // This calls tsGetters.roundTripObject(payload) inside Rust and asserts the result.
  // It will throw if roundTripObject returns the wrong object.
  testRoundTripThroughForeign(tsGetters);
});

(async () => {
  for (let i = 1; i <= 4096; i *= 2) {
    await asyncTest(
      `Full tilt test up to ${i}`,
      async (t) => await testToMax(i, t),
    );
  }
})();
