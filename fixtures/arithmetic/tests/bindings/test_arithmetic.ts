/*
fixture=arithmetic
cargo run --manifest-path ./crates/uniffi-bindgen-react-native/Cargo.toml -- ./fixtures/${fixture}/src/${fixture}.udl --out-dir ./fixtures/${fixture}/generated
cargo xtask run ./fixtures/${fixture}/tests/bindings/test_${fixture}.ts --cpp ./fixtures/${fixture}/generated/${fixture}.cpp --crate ./fixtures/${fixture}
*/
import * as rust from "../../generated/arithmetic";
import { assertEqual } from "@/asserts";
import { console } from "@/hermes";

const a = BigInt(39);
const b = BigInt(3);

console.log(`${a} + ${b} = ${rust.add(a, b)}`);
assertEqual(a + b, rust.add(a, b));
assertEqual(a - b, rust.sub(a, b));
assertEqual(a / b, rust.div(a, b));
