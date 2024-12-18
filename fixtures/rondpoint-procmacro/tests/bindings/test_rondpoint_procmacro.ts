/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// fixture=rondpoint
// cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
// cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}

import { Asserts, test } from "@/asserts";
import {
  Enumeration,
  EnumerationAvecDonnees,
  Optionneur,
  Dictionnaire,
  OptionneurDictionnaire,
  Retourneur,
  Stringifier,
  copieCarte,
  copieDictionnaire,
  copieEnumeration,
  copieEnumerations,
  switcheroo,
} from "../../generated/uniffi_rondpointpm";
import { numberToString } from "@/simulated";

test("Round trip a single enum", (t) => {
  const input = Enumeration.Deux;
  const output = copieEnumeration(input);
  t.assertEqual(input, output);
});

test("Round trip a list of enum", (t) => {
  const input = [Enumeration.Un, Enumeration.Deux];
  const output = copieEnumerations(input);
  t.assertEqual(input, output);
});

test("Round trip an object literal, without strings", (t) => {
  const input: Dictionnaire = Dictionnaire.create({
    un: Enumeration.Deux,
    deux: true,
    petitNombre: 0,
    grosNombre: BigInt("123456789"),
  });
  const output = copieDictionnaire(input);
  t.assertEqual(input, output);
});

test("Round trip a map<string, *> of strings to enums with values", (t) => {
  const input = new Map<string, EnumerationAvecDonnees>([
    ["0", new EnumerationAvecDonnees.Zero()],
    ["1", new EnumerationAvecDonnees.Un({ premier: 1 })],
    ["2", new EnumerationAvecDonnees.Deux({ premier: 2, second: "deux" })],
  ]);
  const output = copieCarte(input);
  t.assertEqual(input, output);
});

test("Round trip a boolean", (t) => {
  const input = false;
  const output = switcheroo(input);
  t.assertNotEqual(input, output);
});

const affirmAllerRetour = <T>(
  t: Asserts,
  fn: (input: T) => T,
  fnName: string,
  inputs: T[],
) => {
  for (const input of inputs) {
    const output = fn(input);
    t.assertEqual(input, output, `${fnName} roundtrip failing`);
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
  enums: [Enumeration.Un, Enumeration.Deux, Enumeration.Trois],
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

test("Using an object to roundtrip primitives", (t) => {
  const rt = new Retourneur();
  t.assertNotNull(rt);
  affirmAllerRetour(
    t,
    rt.identiqueBoolean.bind(rt),
    "identiqueBoolean",
    inputData.boolean,
  );

  // 8 bit
  affirmAllerRetour(t, rt.identiqueI8.bind(rt), "identiqueI8", inputData.i8);
  affirmAllerRetour(t, rt.identiqueU8.bind(rt), "identiqueU8", inputData.u8);

  // 16 bit
  affirmAllerRetour(t, rt.identiqueI16.bind(rt), "identiqueI16", inputData.i16);
  affirmAllerRetour(t, rt.identiqueU16.bind(rt), "identiqueU16", inputData.u16);

  // 32 bits
  affirmAllerRetour(t, rt.identiqueI32.bind(rt), "identiqueI32", inputData.i32);
  affirmAllerRetour(t, rt.identiqueU32.bind(rt), "identiqueU32", inputData.u32);
  affirmAllerRetour(
    t,
    rt.identiqueFloat.bind(rt),
    "identiqueF32",
    inputData.f32,
  );

  // 64 bits
  affirmAllerRetour(t, rt.identiqueI64.bind(rt), "identiqueI64", inputData.i64);
  affirmAllerRetour(t, rt.identiqueU64.bind(rt), "identiqueU64", inputData.u64);
  affirmAllerRetour(
    t,
    rt.identiqueDouble.bind(rt),
    "identiqueF32",
    inputData.f64,
  );

  rt.uniffiDestroy();
});

test("Testing defaulting properties in record types", (t) => {
  const rt = new Retourneur();
  const explicit = OptionneurDictionnaire.create({
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
    // enumerationVar: Enumeration.Deux,
    dictionnaireVar: undefined,
  });
  const defaulted: OptionneurDictionnaire = explicit; //OptionneurDictionnaire.create({});
  t.assertEqual(explicit, defaulted);

  // const actualDefaults = OptionneurDictionnaire.defaults();
  // t.assertEqual(defaulted, actualDefaults);

  affirmAllerRetour(
    t,
    rt.identiqueOptionneurDictionnaire.bind(rt),
    "identiqueOptionneurDictionnaire",
    [explicit],
  );
  rt.uniffiDestroy();
});

test("Using an object to roundtrip strings", (t) => {
  const rt = new Retourneur();
  t.assertNotNull(rt);
  affirmAllerRetour(
    t,
    rt.identiqueString.bind(rt),
    "identiqueString",
    inputData.string,
  );
  rt.uniffiDestroy();
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
  t: Asserts,
  fn: (input: T) => string,
  fnName: string,
  inputs: T[],
  toString: TypescriptToString<T> = (value: T): string => `${value}`,
) {
  for (const input of inputs) {
    const expected = toString(input);
    const observed = fn(input);
    t.assertEqual(expected, observed, `Stringifier ${fnName} failing`);
  }
}

test("Using an object convert into strings", (t) => {
  const st = new Stringifier();
  t.assertNotNull(st);

  const input = "JS on Hermes";
  t.assertEqual(`uniffi 💚 ${input}!`, st.wellKnownString(input));

  affirmEnchaine(
    t,
    st.toStringBoolean.bind(st),
    "toStringBoolean",
    inputData.boolean,
  );
  affirmEnchaine(t, st.toStringI8.bind(st), "toStringI8", inputData.i8);
  affirmEnchaine(t, st.toStringU8.bind(st), "toStringU8", inputData.u8);
  affirmEnchaine(t, st.toStringI16.bind(st), "toStringI16", inputData.i16);
  affirmEnchaine(t, st.toStringU16.bind(st), "toStringU16", inputData.u16);
  affirmEnchaine(t, st.toStringI32.bind(st), "toStringI32", inputData.i32);
  affirmEnchaine(t, st.toStringU32.bind(st), "toStringU32", inputData.u32);
  affirmEnchaine(t, st.toStringI64.bind(st), "toStringI64", inputData.i64);
  affirmEnchaine(t, st.toStringU64.bind(st), "toStringU64", inputData.u64);
  affirmEnchaine(
    t,
    st.toStringFloat.bind(st),
    "toStringFloat",
    inputData.f32,
    numberToString,
  );
  affirmEnchaine(
    t,
    st.toStringDouble.bind(st),
    "toStringDouble",
    inputData.f64,
    numberToString,
  );

  st.uniffiDestroy();
});

test("Default arguments are defaulting", (t) => {
  // Prove to ourselves that default arguments are being used.
  // Step 1: call the methods without arguments, and check against the UDL.
  const op = new Optionneur();

  t.assertEqual(op.sinonString(), "default");
  t.assertEqual(op.sinonBoolean(), false);
  t.assertEqual(op.sinonSequence(), []);

  // optionals
  t.assertEqual(op.sinonNull(), null);
  t.assertEqual(op.sinonZero(), 0);

  // decimal integers
  t.assertEqual(op.sinonI8Dec(), -42);
  t.assertEqual(op.sinonU8Dec(), 42);
  t.assertEqual(op.sinonI16Dec(), 42);
  t.assertEqual(op.sinonU16Dec(), 42);
  t.assertEqual(op.sinonI32Dec(), 42);
  t.assertEqual(op.sinonU32Dec(), 42);
  t.assertEqual(op.sinonI64Dec(), BigInt("42"));
  t.assertEqual(op.sinonU64Dec(), BigInt("42"));

  // hexadecimal integers
  t.assertEqual(op.sinonI8Hex(), -0x7f);
  t.assertEqual(op.sinonU8Hex(), 0xff);
  t.assertEqual(op.sinonI16Hex(), 0x7f);
  t.assertEqual(op.sinonU16Hex(), 0x7f);
  t.assertEqual(op.sinonI32Hex(), 0x7fffffff);
  t.assertEqual(op.sinonU32Hex(), 0xffffffff);
  t.assertEqual(op.sinonI64Hex(), BigInt("0x7fffffffffffffff"));
  t.assertEqual(op.sinonU64Hex(), BigInt("0xffffffffffffffff"));

  // octal integers
  t.assertEqual(op.sinonU32Oct(), 493); // 0o755

  // floats
  t.assertEqual(op.sinonF32(), 42.0);
  t.assertEqual(op.sinonF64(), 42.1);

  // enums
  // t.assertEqual(op.sinonEnum(Enumeration.Trois), Enumeration.Trois);

  op.uniffiDestroy();
});

test("Default arguments are overridden", (t) => {
  // Step 2. Convince ourselves that if we pass something else, then that changes the output.
  //         We have shown something coming out of the sinon methods, but without eyeballing the Rust
  //         we can't be sure that the arguments will change the return value.
  const op = new Optionneur();

  // Now passing an argument, showing that it wasn't hardcoded anywhere.
  affirmAllerRetour(
    t,
    op.sinonBoolean.bind(op),
    "sinonBoolean",
    inputData.boolean,
  );

  // 8 bit
  affirmAllerRetour(t, op.sinonI8Dec.bind(op), "sinonI8Dec", inputData.i8);
  affirmAllerRetour(t, op.sinonI8Hex.bind(op), "sinonI8Hex", inputData.i8);
  affirmAllerRetour(t, op.sinonU8Dec.bind(op), "sinonU8Dec", inputData.u8);
  affirmAllerRetour(t, op.sinonU8Hex.bind(op), "sinonU8Hex", inputData.u8);

  // 16 bit
  affirmAllerRetour(t, op.sinonI16Dec.bind(op), "sinonI16Dec", inputData.i16);
  affirmAllerRetour(t, op.sinonI16Hex.bind(op), "sinonI16Hex", inputData.i16);
  affirmAllerRetour(t, op.sinonU16Dec.bind(op), "sinonU16Dec", inputData.u16);
  affirmAllerRetour(t, op.sinonU16Hex.bind(op), "sinonU16Hex", inputData.u16);

  // 32 bits
  affirmAllerRetour(t, op.sinonI32Dec.bind(op), "sinonI32Dec", inputData.i32);
  affirmAllerRetour(t, op.sinonI32Hex.bind(op), "sinonI32Hex", inputData.i32);
  affirmAllerRetour(t, op.sinonU32Dec.bind(op), "sinonU32Dec", inputData.u32);
  affirmAllerRetour(t, op.sinonU32Hex.bind(op), "sinonU32Hex", inputData.u32);
  affirmAllerRetour(t, op.sinonU32Oct.bind(op), "sinonU32Oct", inputData.u32);
  // 32 bit float
  affirmAllerRetour(t, op.sinonF32.bind(op), "sinonF32", inputData.f32);

  // 64 bits
  affirmAllerRetour(t, op.sinonI64Dec.bind(op), "sinonI64Dec", inputData.i64);
  affirmAllerRetour(t, op.sinonI64Hex.bind(op), "sinonI64Hex", inputData.i64);
  affirmAllerRetour(t, op.sinonU64Dec.bind(op), "sinonU64Dec", inputData.u64);
  affirmAllerRetour(t, op.sinonU64Hex.bind(op), "sinonU64Hex", inputData.u64);

  // 64 bit float
  affirmAllerRetour(t, op.sinonF64.bind(op), "sinonF64", inputData.f64);

  // affirmAllerRetour(t, op.sinonEnum.bind(op), "sinonEnum", inputData.enums);

  op.uniffiDestroy();
});
