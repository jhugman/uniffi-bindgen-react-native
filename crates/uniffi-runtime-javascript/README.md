# `uniffi-runtime-javascript`

The command line tool `uniffi-bindgen-react-native` can generate Javascript bindings crate for your
Rust crate, from `[uniffi::export]` proc-macros in your code.

These bindings crates depend on this crate. Currently this is for WASM runtimes only.

This crate is not intended to be used directly by hand-written code.

If you find this crate from a `Cargo.toml`, it is almost certain that the crate will have
been generated.

For more information visit: https://jhugman.github.io/uniffi-bindgen-react-native

# What is `uniffi`?

`uniffi` is a multi-language bindings generator for Rust, started at, and maintained by Mozilla.

For more information visit: https://mozilla.github.io/uniffi-rs

Most uniffi generated bindings go through a C ABI, called through different languages C FFI facilities. This crate is the beginnings of using Rust to call in to the C ABI.

## Features

The crate is split into features, one per Javascript runtime:

- `wasm32`: uses `wasm-bindgen`.

Future features may be:

- `napi`: which uses `napi-rs` to provide node bindings.

## Generating `uniffi` based bindings for other Javascript Runtimes without this crate

- [`uniffi-bindgen-react-native`][ubrn] generates C++ for the Hermes JS engine used by React Native.
- internally, Firefox uses [uniffi to generate bindings for privileged JS][uniffi-gecko-js].

[ubrn]: https://www.npmjs.com/package/uniffi-bindgen-react-native
[uniffi-gecko-js]: https://firefox-source-docs.mozilla.org/rust-components/developing-rust-components/uniffi.html
