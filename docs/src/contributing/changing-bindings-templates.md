# Changing generated Typescript or C++ bindings templates

The central workings of a `uniffi-bingen` are its templates.

`uniffi-bindgen-react-native` templates are in the following directories:

- [Typescript templates][ts-templates]
- [C++ templates][cpp-templates]

Templates are written for [Rinja templating library](https://rinja.readthedocs.io/en/stable/template_syntax.html).

The WASM crate is generated with `quote`, in the [`gen_rust`][rust-templates] module.

There is a small-ish runtime per target:

- [`typescript/src`][ts-runtime], with [tests][ts-tests] and [polyfills][ts-polyfills]. This is the TypeScript runtime, published to npm as [`@ubjs/core`][core-runtime] and shared by all targets.
- ['cpp/includes`][cpp-runtime], the C++/JSI runtime for React Native, published as the `uniffi-bindgen-react-native.podspec`.
- [`runtimes/napi`][napi-runtime], the N-API runtime for Node.js, published as [`@ubjs/node`][napi-npm]. Its Rust core is in [`runtimes/core`][napi-core].

This is intended to allow developers from outside the project to contribute more easily.

Making a change to the templates should be accompanied by an additional test, either in [an existing test fixture][fixtures], or a new one.

Running the tests can be done with:

```sh
cargo test
```

An individual fixture can be tested:

```sh
cargo test -p uniffi-fixture-$fixtureName
```

[ts-templates]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_bindgen/src/bindings/react_native/gen_typescript/templates
[cpp-templates]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_bindgen/src/bindings/react_native/gen_cpp/templates
[rust-templates]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_bindgen/src/bindings/react_native/gen_cpp/mod.rs
[ts-runtime]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/src
[ts-tests]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/tests
[ts-polyfills]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/testing
[cpp-runtime]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/cpp/includes
[core-runtime]: https://www.npmjs.com/package/@ubjs/core
[napi-runtime]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/runtimes/napi
[napi-npm]: https://www.npmjs.com/package/@ubjs/node
[napi-core]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/runtimes/core
[fixtures]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures
