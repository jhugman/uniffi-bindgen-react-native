# Changing generated Typescript or C++ templates

The central workings of a `uniffi-bingen` are its templates.

`uniffi-bindgen-react-native` templates are in the following directories:

- [Typescript templates][ts-templates]
- [C++ templates][cpp-templates]

Templates are written for [Askama templating library](https://djc.github.io/askama/template_syntax.html).

There is a small-ish runtime for both languages:

- [`typescript/src`][ts-runtime], with [tests][ts-tests] and [polyfills][ts-polyfills].
- ['cpp/includes`][cpp-runtime].

This is intended to allow developers from outside the project to contribute more easily.

Making a change to the templates should be accompanied by an additional test, either in [an existing test fixture][fixtures], or a new one.

Running the tests can be done with:

```sh
./scripts/run-tests.sh
```

An individual test can be run:

```sh
./scripts/run-tests.sh -f $fixtureName
```

[ts-templates]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_bindgen/src/bindings/react_native/gen_typescript/templates
[cpp-templates]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_bindgen/src/bindings/react_native/gen_cpp/templates
[ts-runtime]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/src
[ts-tests]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/tests
[ts-polyfills]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/typescript/testing
[cpp-runtime]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/cpp/includes
[fixtures]: https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures
