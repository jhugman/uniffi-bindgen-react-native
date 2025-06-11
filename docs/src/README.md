[`uniffi-rs`](https://github.com/mozilla/uniffi-rs/blob/main/README.md) is a suite of projects to allow Rust to be used from other languages. It was started at Mozilla to facilitate building cross-platform components in Rust which could be run on Android and iOS.

It has since grown to support for other languages not in use at Mozilla.

![React Native Logo](images/react-native-logo.svg)
↔️
![Rust Logo](images/rust-logo.svg)

![Typescript logo](images/typescript-logo.svg)
↔️
![WASM logo](images/wasm-logo.svg)

[`uniffi-bindgen-react-native`](https://github.com/jhugman/uniffi-bindgen-react-native) is the project that houses the bindings generators for WASM and React Native.

It supports all language features that `uniffi-rs` supports, including:

- calling functions from Typescript to Rust, synchronous and asynchronous.
- calling functions from Rust to Typescript, synchronous and asynchronous.
- objects with methods, including:
    - garbage collection integration.
    - uniffi traits
- custom types

It contains tooling to generate bindings:

- for Hermes via JSI, and to generate the code to create turbo-modules.
- for WASM, using wasm-bindgen, and the WASM crate.

```admonish warning
This project is still in early development, and should not yet be used in production.
```
