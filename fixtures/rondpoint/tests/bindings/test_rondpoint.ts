/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// fixture=rondpoint
// cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
// cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}

import { assertEqual, assertNotEqual, assertNotNull, test } from "@/asserts";
import {
  Enumeration,
  type EnumerationAvecDonnees,
  EnumerationAvecDonneesKind,
  Optionneur,
  OptionneurDictionnaire,
  Retourneur,
  Stringifier,
  copieCarte,
  copieDictionnaire,
  copieEnumeration,
  copieEnumerations,
  createDictionnaire,
  createOptionneurDictionnaire,
  switcheroo,
} from "../../generated/rondpoint";
import { numberToString } from "@/simulated";

test("Round trip a single enum", () => {
  const input = Enumeration.DEUX;
  const output = copieEnumeration(input);
  assertEqual(input, output);
});

test("Round trip a list of enum", () => {
  const input = [Enumeration.UN, Enumeration.DEUX];
  const output = copieEnumerations(input);
  assertEqual(input, output);
});

test("Round trip an object literal, without strings", () => {
  const input = createDictionnaire({
    un: Enumeration.DEUX,
    deux: true,
    petitNombre: 0,
    grosNombre: BigInt("123456789"),
  });
  const output = copieDictionnaire(input);
  assertEqual(input, output);
});

test("Round trip a map<string, *> of strings to enums with values", () => {
  const input = new Map<string, EnumerationAvecDonnees>([
    ["0", { kind: EnumerationAvecDonneesKind.ZERO }],
    ["1", { kind: EnumerationAvecDonneesKind.UN, value: { premier: 1 } }],
    [
      "2",
      {
        kind: EnumerationAvecDonneesKind.DEUX,
        value: { premier: 2, second: "deux" },
      },
    ],
  ]);
  const output = copieCarte(input);
  assertEqual(input, output);
});

test("Round trip a boolean", () => {
  const input = false;
  const output = switcheroo(input);
  assertNotEqual(input, output);
});

const affirmAllerRetour = <T>(
  fn: (input: T) => T,
  fnName: string,
  inputs: T[],
) => {
  for (const input of inputs) {
    const output = fn(input);
    assertEqual(input, output, `${fnName} roundtrip failing`);
  }
};

const inputData = {
  boolean: [true, false],
  i8: [-0x7f, 0, 0x7f],
  u8: [0, 0xff],
  i16: [-0x7fff, 0, 0x7fff],
  u16: [0, 0xffff],
  i32: [-0x7fffffff, 0, 0x7fffffff],
  u32: [0, 0xffffffff],
  i64: [
    -BigInt("0x7fffffffffffffff"),
    BigInt("0"),
    BigInt("0x7fffffffffffffff"),
  ],
  u64: [BigInt("0"), BigInt("0xffffffffffffffff")],
  f32: [3.5, 27, -113.75, 0.0078125, 0.5, 0, -1],
  f64: [Number.MIN_VALUE, Number.MAX_VALUE],
  enums: [Enumeration.UN, Enumeration.DEUX, Enumeration.TROIS],
  string: [
    "",
    "abc",
    "null\u0000byte",
    "été",
    "ښي لاس ته لوستلو لوستل",
    "😻emoji 👨‍👧‍👦multi-emoji, 🇨🇭a flag, a canal, panama",
  ],
};

// Test the roundtrip across the FFI.
// This shows that the values we send come back in exactly the same state as we sent them.
// i.e. it shows that lowering from kotlin and lifting into rust is symmetrical with
//      lowering from rust and lifting into typescript.

test("Using an object to roundtrip primitives", () => {
  const rt = new Retourneur();
  assertNotNull(rt);
  affirmAllerRetour(
    rt.identiqueBoolean.bind(rt),
    "identiqueBoolean",
    inputData.boolean,
  );

  // 8 bit
  affirmAllerRetour(rt.identiqueI8.bind(rt), "identiqueI8", inputData.i8);
  affirmAllerRetour(rt.identiqueU8.bind(rt), "identiqueU8", inputData.u8);

  // 16 bit
  affirmAllerRetour(rt.identiqueI16.bind(rt), "identiqueI16", inputData.i16);
  affirmAllerRetour(rt.identiqueU16.bind(rt), "identiqueU16", inputData.u16);

  // 32 bits
  affirmAllerRetour(rt.identiqueI32.bind(rt), "identiqueI32", inputData.i32);
  affirmAllerRetour(rt.identiqueU32.bind(rt), "identiqueU32", inputData.u32);
  affirmAllerRetour(rt.identiqueFloat.bind(rt), "identiqueF32", inputData.f32);

  // 64 bits
  affirmAllerRetour(rt.identiqueI64.bind(rt), "identiqueI64", inputData.i64);
  affirmAllerRetour(rt.identiqueU64.bind(rt), "identiqueU64", inputData.u64);
  affirmAllerRetour(rt.identiqueDouble.bind(rt), "identiqueF32", inputData.f64);

  rt.destroy();
});

test("Testing defaulting properties in record types", () => {
  const rt = new Retourneur();
  const defaults: OptionneurDictionnaire = createOptionneurDictionnaire({});
  const explicit = createOptionneurDictionnaire({
    i8Var: -8,
    u8Var: 8,
    i16Var: -16,
    u16Var: 0x10,
    i32Var: -32,
    u32Var: 32,
    i64Var: -BigInt("64"),
    u64Var: BigInt("64"),
    floatVar: 4.0,
    doubleVar: 8.0,
    booleanVar: true,
    stringVar: "default",
    listVar: [],
    enumerationVar: Enumeration.DEUX,
    dictionnaireVar: undefined,
  });
  assertEqual(explicit, defaults);

  affirmAllerRetour(
    rt.identiqueOptionneurDictionnaire.bind(rt),
    "identiqueOptionneurDictionnaire",
    [explicit],
  );
  rt.destroy();
});

test("Using an object to roundtrip strings", () => {
  const rt = new Retourneur();
  assertNotNull(rt);
  affirmAllerRetour(
    rt.identiqueString.bind(rt),
    "identiqueString",
    inputData.string,
  );
  rt.destroy();
});

// Test one way across the FFI.
//
// We send one representation of a value to lib.rs, and it transforms it into another, a string.
// lib.rs sends the string back, and then we compare here in kotlin.
//
// This shows that the values are transformed into strings the same way in both kotlin and rust.
// i.e. if we assume that the string return works (we test this assumption elsewhere)
//      we show that lowering from kotlin and lifting into rust has values that both kotlin and rust
//      both stringify in the same way. i.e. the same values.
//
// If we roundtripping proves the symmetry of our lowering/lifting from here to rust, and lowering/lifting from rust t here,
// and this convinces us that lowering/lifting from here to rust is correct, then
// together, we've shown the correctness of the return leg.

type TypescriptToString<T> = (value: T) => string;
function affirmEnchaine<T>(
  fn: (input: T) => string,
  fnName: string,
  inputs: T[],
  toString: TypescriptToString<T> = (value: T): string => `${value}`,
) {
  for (const input of inputs) {
    const expected = toString(input);
    const observed = fn(input);
    assertEqual(expected, observed, `Stringifier ${fnName} failing`);
  }
}

test("Using an object convert into strings", () => {
  const st = new Stringifier();
  assertNotNull(st);

  const input = "JS on Hermes";
  assertEqual(`uniffi 💚 ${input}!`, st.wellKnownString(input));

  affirmEnchaine(
    st.toStringBoolean.bind(st),
    "toStringBoolean",
    inputData.boolean,
  );
  affirmEnchaine(st.toStringI8.bind(st), "toStringI8", inputData.i8);
  affirmEnchaine(st.toStringU8.bind(st), "toStringU8", inputData.u8);
  affirmEnchaine(st.toStringI16.bind(st), "toStringI16", inputData.i16);
  affirmEnchaine(st.toStringU16.bind(st), "toStringU16", inputData.u16);
  affirmEnchaine(st.toStringI32.bind(st), "toStringI32", inputData.i32);
  affirmEnchaine(st.toStringU32.bind(st), "toStringU32", inputData.u32);
  affirmEnchaine(st.toStringI64.bind(st), "toStringI64", inputData.i64);
  affirmEnchaine(st.toStringU64.bind(st), "toStringU64", inputData.u64);
  affirmEnchaine(
    st.toStringFloat.bind(st),
    "toStringFloat",
    inputData.f32,
    numberToString,
  );
  affirmEnchaine(
    st.toStringDouble.bind(st),
    "toStringDouble",
    inputData.f64,
    numberToString,
  );

  st.destroy();
});

test("Default arguments are defaulting", () => {
  // Prove to ourselves that default arguments are being used.
  // Step 1: call the methods without arguments, and check against the UDL.
  const op = new Optionneur();

  assertEqual(op.sinonString(), "default");
  assertEqual(op.sinonBoolean(), false);
  assertEqual(op.sinonSequence(), []);

  // optionals
  assertEqual(op.sinonNull(), null);
  assertEqual(op.sinonZero(), 0);

  // decimal integers
  assertEqual(op.sinonI8Dec(), -42);
  assertEqual(op.sinonU8Dec(), 42);
  assertEqual(op.sinonI16Dec(), 42);
  assertEqual(op.sinonU16Dec(), 42);
  assertEqual(op.sinonI32Dec(), 42);
  assertEqual(op.sinonU32Dec(), 42);
  assertEqual(op.sinonI64Dec(), BigInt("42"));
  assertEqual(op.sinonU64Dec(), BigInt("42"));

  // hexadecimal integers
  assertEqual(op.sinonI8Hex(), -0x7f);
  assertEqual(op.sinonU8Hex(), 0xff);
  assertEqual(op.sinonI16Hex(), 0x7f);
  assertEqual(op.sinonU16Hex(), 0xffff);
  assertEqual(op.sinonI32Hex(), 0x7fffffff);
  assertEqual(op.sinonU32Hex(), 0xffffffff);
  assertEqual(op.sinonI64Hex(), BigInt("0x7fffffffffffffff"));
  assertEqual(op.sinonU64Hex(), BigInt("0xffffffffffffffff"));

  // octal integers
  assertEqual(op.sinonU32Oct(), 493); // 0o755

  // floats
  assertEqual(op.sinonF32(), 42.0);
  assertEqual(op.sinonF64(), 42.1);

  // enums
  assertEqual(op.sinonEnum(), Enumeration.TROIS);

  op.destroy();
});

test("Default arguments are overridden", () => {
  // Step 2. Convince ourselves that if we pass something else, then that changes the output.
  //         We have shown something coming out of the sinon methods, but without eyeballing the Rust
  //         we can't be sure that the arguments will change the return value.
  const op = new Optionneur();

  // Now passing an argument, showing that it wasn't hardcoded anywhere.
  affirmAllerRetour(
    op.sinonBoolean.bind(op),
    "sinonBoolean",
    inputData.boolean,
  );

  // 8 bit
  affirmAllerRetour(op.sinonI8Dec.bind(op), "sinonI8Dec", inputData.i8);
  affirmAllerRetour(op.sinonI8Hex.bind(op), "sinonI8Hex", inputData.i8);
  affirmAllerRetour(op.sinonU8Dec.bind(op), "sinonU8Dec", inputData.u8);
  affirmAllerRetour(op.sinonU8Hex.bind(op), "sinonU8Hex", inputData.u8);

  // 16 bit
  affirmAllerRetour(op.sinonI16Dec.bind(op), "sinonI16Dec", inputData.i16);
  affirmAllerRetour(op.sinonI16Hex.bind(op), "sinonI16Hex", inputData.i16);
  affirmAllerRetour(op.sinonU16Dec.bind(op), "sinonU16Dec", inputData.u16);
  affirmAllerRetour(op.sinonU16Hex.bind(op), "sinonU16Hex", inputData.u16);

  // 32 bits
  affirmAllerRetour(op.sinonI32Dec.bind(op), "sinonI32Dec", inputData.i32);
  affirmAllerRetour(op.sinonI32Hex.bind(op), "sinonI32Hex", inputData.i32);
  affirmAllerRetour(op.sinonU32Dec.bind(op), "sinonU32Dec", inputData.u32);
  affirmAllerRetour(op.sinonU32Hex.bind(op), "sinonU32Hex", inputData.u32);
  affirmAllerRetour(op.sinonU32Oct.bind(op), "sinonU32Oct", inputData.u32);
  // 32 bit float
  affirmAllerRetour(op.sinonF32.bind(op), "sinonF32", inputData.f32);

  // 64 bits
  affirmAllerRetour(op.sinonI64Dec.bind(op), "sinonI64Dec", inputData.i64);
  affirmAllerRetour(op.sinonI64Hex.bind(op), "sinonI64Hex", inputData.i64);
  affirmAllerRetour(op.sinonU64Dec.bind(op), "sinonU64Dec", inputData.u64);
  affirmAllerRetour(op.sinonU64Hex.bind(op), "sinonU64Hex", inputData.u64);

  // 64 bit float
  affirmAllerRetour(op.sinonF64.bind(op), "sinonF64", inputData.f64);

  affirmAllerRetour(op.sinonEnum.bind(op), "sinonEnum", inputData.enums);

  op.destroy();
});
