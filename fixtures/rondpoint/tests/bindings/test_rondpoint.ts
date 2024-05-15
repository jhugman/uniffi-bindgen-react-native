/*
fixture=rondpoint
cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}
*/
import { assertEqual, assertNotEqual, assertNotNull, test } from "@/asserts";
import {
  Enumeration,
  EnumerationAvecDonnees,
  Retourneur,
  Stringifier,
  copieCarte,
  copieDictionnaire,
  copieEnumeration,
  copieEnumerations,
  createDictionnaire,
} from "../../generated/rondpoint";

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
  const input = new Map([
    ["0", new EnumerationAvecDonnees.Zero()],
    ["1", new EnumerationAvecDonnees.Un({ premier: 1 })],
    ["2", new EnumerationAvecDonnees.Deux({ premier: 2, second: "deux" })],
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
  string: [
    "",
    "abc",
    "null\u0000byte",
    "été",
    "ښي لاس ته لوستلو لوستل",
    "😻emoji 👨‍👧‍👦multi-emoji, 🇨🇭a flag, a canal, panama",
  ],
};

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

  // 64 bits
  affirmAllerRetour(rt.identiqueI64.bind(rt), "identiqueI64", inputData.i64);
  affirmAllerRetour(rt.identiqueU64.bind(rt), "identiqueU64", inputData.u64);

  rt.destroy();
});

test("Using an object to roundtrip strings", () => {
  const rt = new Retourneur();
  assertNotNull(rt);
  affirmAllerRetour(rt.identiqueString.bind(rt), "identiqueString", inputData.string);
  rt.destroy();
});

test("Using an object convert into strings", () => {
  const st = new Stringifier();
  assertNotNull(st);

  const input = "JS on Hermes";
  assertEqual(`uniffi 💚 ${input}!`, st.wellKnownString(input));

  st.destroy();
});
