[![CI](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/ci.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/ci.yml)
[![Compatibility (Android)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android.yml)
[![Compatibility (iOS)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios.yml)
[![Compatibility (Android, latest)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android-latest.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android-latest.yml)
[![Compatibility (iOS, latest)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios-latest.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios-latest.yml)

# uniffi-bindgen-react-native
[UniFFI](https://mozilla.github.io/uniffi-rs/latest/) is a multi-language bindings generator for Rust.

This project, `uniffi-bindgen-react-native`, is a uniFFI bindings generator for using Rust from React Native.

It provides tooling to generate:

- Typescript and JSI C++ to call Rust from Typescript and back again
- a Turbo-Module that installs the bindings into a running React Native library.

If you're ready to start, then start with [a step-by-step tutorial to make a Rust turbo-module](https://jhugman.github.io/uniffi-bindgen-react-native/).

If you're new to uniFFI, then [**the UniFFI user guide**](https://mozilla.github.io/uniffi-rs/latest/)
or [**the UniFFI examples**](https://github.com/mozilla/uniffi-rs/tree/main/examples#example-uniffi-components) are interesting places to start.

## Why `uniffi-bindgen-react-native`?

- Spend more time writing Typescript and Rust
- Full compatibility with `uniffi-rs`
- Your Rust SDK is portable across multiple languages.

### Why not, say WASM, via `wasm-bindgen`?

WASM is an amazing virtual machine however:

- your Rust crate must make alternative arrangements if it needs things that the virtual machine does not offer:
    - threads and
    - file access.
- you need to maintain a separate FFI (this is a temporary issue, solvable by something like uniFFI).

## Who is using `uniffi-bindgen-react-native`?

- [@unomed/react-native-matrix-sdk](https://www.npmjs.com/package/@unomed/react-native-matrix-sdk)
- [ChessTiles on iOS](https://apps.apple.com/us/app/chesstiles/id6737867924) "uniffi-bindgen-react-native lets us run our performance critical solution search algorithm and business logic in Rust, while rapidly prototying the UI with React Native"

## Prior art and related projects

- [cawfree/react-native-webassembly](https://github.com/cawfree/react-native-webassembly)

## Contributing

If this tool sounds interesting to you, please help us develop it! You can:

* View the [contributor guidelines](https://jhugman.github.io/uniffi-bindgen-react-native/).
* File or work on [issues](https://github.com/jhugman/uniffi-bindgen-react-native/issues) here in GitHub.
* Join discussions in the [#uniffi-bindgen-js:matrix.org](https://matrix.to/#/#uniffi-bindgen-js:matrix.org) room on Matrix.

## Code of Conduct

This project is governed by Mozilla's [Community Participation Guidelines](./CODE_OF_CONDUCT.md).

## Funding

`uniffi-bindgen-react-native` is led by James Hugman, with deep collaboration from the [Filament](https://filament.im) engineering team, funded by [Filament](https://filament.im) and [Mozilla](https://future.mozilla.org).

## License

[MPL-2.0](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/LICENSE)
