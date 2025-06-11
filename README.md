[![CI](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/ci.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/ci.yml)
[![Compatibility (Android)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android.yml)
[![Compatibility (iOS)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios.yml)
[![Compatibility (Android, latest)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android-latest.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android-latest.yml)
[![Compatibility (iOS, latest)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios-latest.yml/badge.svg)](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios-latest.yml)

# 🦀 uniffi-bindgen-react-native

`uniffi-bindgen-react-native` is a tool that generates TypeScript bindings for Rust code, making it usable in React Native apps and web pages. It builds on [UniFFI](https://mozilla.github.io/uniffi-rs/latest/), Mozilla's bindings generator ecosystem.

With this tool, you can write your business logic once in Rust and access it seamlessly from TypeScript, whether you're developing for mobile platforms or the web.

UniFFI provides procedural macros to describe your API, prioritizing expressivity and memory safety. This makes it ideal for portability.

## ✨ Features

It provides tooling to generate safe and performant TypeScript to access Rust from:

- 📱 **React Native**
  - with JSI C++ to call Rust from TypeScript and back again, and
  - a Turbo-Module that installs the bindings into a running React Native library.
- 🌐 **Web pages**
  - with a WASM binding crate

All using the same proc macros: you annotate your Rust once, and build for Android, iOS and the Web.

Javascript hosts the Rust library, and `uniffi-bindgen-react-native` and `uniffi` facilitate the communication between the two:

- Same thread calling across the FFI from Javascript to Rust.
- Async calls from Javascript to Rust
- Same thread callbacks from Rust to Javascript
- Async callbacks from Rust to Javascript
- Pass by Reference (for "Objects")
- Pass by Value (for "Records")
- Enums and tagged unions

## Why use `uniffi-bindgen-react-native` instead of `wasm-bindgen`?

- `uniffi-bindgen-react-native` _generates_ a `wasm-bindgen` crate, from `uniffi` annotations.
- when you come to use your Rust crate in another context (say, from Python, or Kotlin, or React Native), then you can generate FFIs for those platforms, all with the same `uniffi` annotations.

## 🚀 Getting Started

If you're ready to start, then begin with a step-by-step tutorial to [make a Rust turbo-module](https://jhugman.github.io/uniffi-bindgen-react-native/guides/rn/getting-started.html) and then [run it in web page with WASM](https://jhugman.github.io/uniffi-bindgen-react-native/guides/rn/getting-started.html).

If you're new to UniFFI, then [**the UniFFI user guide**](https://mozilla.github.io/uniffi-rs/latest/)
or [**the UniFFI examples**](https://github.com/mozilla/uniffi-rs/tree/main/examples#example-uniffi-components) are interesting places to start.

## 🤔 Why `uniffi-bindgen-react-native`?

- 🧩 Spend more time writing TypeScript and Rust, less time hand-writing FFIs
- 🌍 Your Rust SDK is portable across multiple languages

## Who is using `uniffi-bindgen-react-native`?

- [@unomed/react-native-matrix-sdk](https://www.npmjs.com/package/@unomed/react-native-matrix-sdk)
- [ChessTiles on iOS](https://apps.apple.com/us/app/chesstiles/id6737867924) "uniffi-bindgen-react-native lets us run our performance critical solution search algorithm and business logic in Rust, while rapidly prototying the UI with React Native"

## Prior art and related projects

- [cawfree/react-native-webassembly](https://github.com/cawfree/react-native-webassembly)

## Notice

Now `uniffi-bindgen-react-native` supports WASM, the `React Native` no longer seems appropriate. In the near future, we'll change the name to `uniffi-bindgen-javascript`. Backwards compatibility will be ensured.

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
