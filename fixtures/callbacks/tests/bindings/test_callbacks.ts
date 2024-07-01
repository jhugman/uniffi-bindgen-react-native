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
} from "../../generated/callbacks";
import {
  assertEqual,
  assertNotEqual,
  assertNotNull,
  test,
  xtest,
} from "@/asserts";
import { console } from "@/hermes";

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
      throw new SimpleException.BadArgument("bad argument");
    }
    if (v == UNEXPECTED_ERROR) {
      throw Error("something failed");
    }
    return arg2 ? v?.toUpperCase() : v;
  }
  getList(v: number[], arg2: boolean): number[] {
    return arg2 ? v : [];
  }
  getNothing(v: string): void {
    if (v == BAD_ARGUMENT) {
      throw new SimpleException.BadArgument("bad argument");
    }
    if (v == UNEXPECTED_ERROR) {
      throw new Error("something failed");
    }
  }
  setNothing(): void {
    console.log("TS: setNothing called!");
  }
}

test("Set nothing values passed between callback interfaces", () => {
  console.log("Starting test");
  const rg = new RustGetters();
  const callbackInterface = new TypeScriptGetters();
  for (const v of inputData.boolean) {
    rg.setNothing(callbackInterface);
  }
  rg.uniffiDestroy();
});

xtest("Boolean values passed between callback interfaces", () => {
  console.log("Starting test");
  const rg = new RustGetters();
  console.log("Made RustGetters");
  const callbackInterface = new TypeScriptGetters();
  console.log("Made ForeignGetters");
  const flag = true;
  for (const v of inputData.boolean) {
    const expected = callbackInterface.getBool(v, flag);
    console.log(`\texpected = ${expected}`);
    const observed = rg.getBool(callbackInterface, v, flag);
    console.log(`\tobserved = ${observed}`);
    assertEqual(observed, expected);
  }
  rg.uniffiDestroy();
});

xtest("List values passed between callback interfaces", () => {
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

xtest("String values passed between callback interfaces", () => {
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
