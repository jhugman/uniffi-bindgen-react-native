/*
fixture=rondpoint
cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}
*/
import { assertEqual, assertNotEqual, test } from "@/asserts";
import {
  Enumeration,
  EnumerationAvecDonnees,
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
  const input = createDictionnaire(
    Enumeration.DEUX,
    true,
    0,
    BigInt("123456789"),
  );
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
