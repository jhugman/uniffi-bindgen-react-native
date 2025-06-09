# Troubleshooting

The diversity of web stacks available mean that we cannot enumerate all the possible errors or problems you may encounter.

However, here are some that the authors have found, and how to fix them.

## `panic!` not reporting, or `RuntimeError: Unreachable executed`

By default, WASM doesn't report to the console when a panic occurs in the Rust.

`wasm-bindgen` provide a [`console_error_panic_hook` crate](https://crates.io/crates/console_error_panic_hook).

You should add this to your target crate's `Cargo.toml`,

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
console_error_panic_hook = "0.1.7"
```

and some place near your Rust startup run:
```rust
#[cfg(target_arch="wasm32")]
console_error_panic_hook::set_once();
```

## `FinalizationRegistry` not found

This occurs when type checking the generated code. It's caused by Typescript not knowing about global classes introduced "recently".

The fix is to update the `tsconfig.json` file's `target` to something more recent than `es2021`.

```json
"compilerOptions":
  "target": "es2021" # or `esnext`
```
