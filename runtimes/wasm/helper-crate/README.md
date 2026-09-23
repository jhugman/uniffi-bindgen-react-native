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
uniffi-runtime-wasm = "0.31.0-5"

[dependencies]
uniffi_core = { version = "0.31", features = ["wasm-unstable-single-threaded"] }
```

The dependency on its own is not enough. Nothing in your crate calls into this
one, so rustc drops the unused rlib and its `#[no_mangle]` exports never reach
the cdylib. Reference it for its side effects in your `lib.rs`:

```rust
#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;
```

Without that line the build succeeds and the module loads, but the player
fails at `uniffiInitAsync` with `required export "__ubrn_alloc" not found in
wasm module`.

`ubrn build wasm2` checks the manifest for all three of these and explains what
is missing. It does not yet check that the exports survived linking.

The JavaScript half ships separately, as [`@ubjs/wasm`][npm] on npm.

## Optional JSPI entry

For generated `wasm2` bindings with `bindings.typescript.jspi` enabled, export
the instrumented entry once from the final cdylib:

```rust
#[cfg(target_arch = "wasm32")]
uniffi_runtime_wasm::export_jspi_entry!();
```

The consuming crate must directly depend on a JSPI-capable `wasm-bindgen`
(tested with `0.2.128`), and staging must use the matching CLI. The macro expands
there so this helper does not pin or upgrade wasm-bindgen for every consumer.
The player calls this unsafe ABI entry with validated signature thunks and
independently owned call frames. Do not call it directly with arbitrary table
indices or pointers. The initial wasm2 JSPI selection supports synchronous
top-level functions; lifecycle exports and Rust async polling remain synchronous.

[npm]: https://www.npmjs.com/package/@ubjs/wasm
