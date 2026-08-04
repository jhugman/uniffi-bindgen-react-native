# uniffi-runtime-wasm

Runtime helpers for the wasm2 player in
[`uniffi-bindgen-react-native`][bindgen]. Add it to a crate you are building
for `wasm32-unknown-unknown`; you do not call it yourself.

[bindgen]: https://github.com/jhugman/uniffi-bindgen-react-native

The crate exports two things from your `cdylib` that the JavaScript side needs
and cannot supply itself: an allocator for linear-memory scratch space, and a
panic hook that forwards Rust panics to a JS function the player installs in
`__indirect_function_table`.

## Usage

```toml
[lib]
crate-type = ["lib", "cdylib"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
uniffi-runtime-wasm = "0.31.0-3"

[dependencies]
uniffi_core = { version = "0.31", features = ["wasm-unstable-single-threaded"] }
```

`ubrn build wasm2` checks all three of these and explains what is missing.

The JavaScript half ships separately, as [`@ubjs/wasm`][npm] on npm.

[npm]: https://www.npmjs.com/package/@ubjs/wasm
