# Packages

`uniffi-bindgen-react-native` is a command line tool, `ubrn`, that turns the `[uniffi::export]` proc-macros in your Rust crate into TypeScript (and, for React Native, C++). The generated code is specific to your library, and leans on a small, fixed runtime — the FFI converters, marshalling, callback dispatch and library loading — that is the same for every library and so is shipped separately.

## What a new project installs

The runtime is published under the `@ubjs` scope. A freshly generated project depends on:

| Target | Install | Imported by |
| ------ | ------- | ----------- |
| Any target | [`@ubjs/core`][core-npm] | the generated TypeScript |
| Node.js | [`@ubjs/node`][napi-npm] (with `@ubjs/core`) | the generated TypeScript |
| React Native | `uniffi-bindgen-react-native` (with `@ubjs/core`) | CocoaPods / CMake, for the C++/JSI runtime |

- **`@ubjs/core`** is the TypeScript runtime (FFI converters, `RustBuffer`, polyfills), shared by every target. New generated code imports it.
- **`@ubjs/node`** is a single prebuilt N-API addon that loads your compiled `cdylib` at runtime and calls into it with [libffi](https://sourceware.org/libffi/). Because UniFFI uses a small, fixed set of FFI types, one addon works with any UniFFI library — there is no per-library glue to compile. See the [Node.js reference](reference/nodejs.md).
- For **React Native**, the C++/JSI runtime ships inside the `uniffi-bindgen-react-native` package (the `uniffi-bindgen-react-native.podspec` and `cpp/includes`), which the generated turbo-module compiles against. Start with the [React Native tutorial](guides/rn/getting-started.md); the [Web tutorial](guides/web/getting-started.md) extends it.

The `uniffi-bindgen-react-native` package itself is the `ubrn` command line and the React Native build tooling. On React Native it is a regular dependency (for the C++ runtime above); for a Node.js-only project it is only needed at build time, as a dev dependency.

## Existing projects keep working

`@ubjs/core` is new. Code generated before it existed imports the runtime from `uniffi-bindgen-react-native` instead, and that package still ships the same runtime bytes under its old name — so projects that haven't regenerated need no changes. The two are version-locked.

[core-npm]: https://www.npmjs.com/package/@ubjs/core
[napi-npm]: https://www.npmjs.com/package/@ubjs/node
