# Adding or changing turbo-module templates

In addition to generating the bindings between Hermes and Rust, `uniffi-bindgen-react-native` generates the files needed to run this as a turbo-module. The list of files are [documented elsewhere in this book](../reference/turbo-module-files.md).

Templates are written for [Askama templating library](https://djc.github.io/askama/template_syntax.html).

Changing the templates for these files is relatively simple. [This PR is a good example](https://github.com/jhugman/uniffi-bindgen-react-native/pull/112) of adding a file.

- Template files are in the [`codegen/templates` directory][codegen/templates].
- Template configuration are in [`codegen/mod.rs`][codegen/mod.rs] file.

[codegen/mod.rs]: https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_cli/src/codegen/mod.rs
[codegen/templates]: https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_cli/src/codegen/templates

### Adding a new template

1. Add new template to the [`codegen/templates` directory][codegen/templates].
1. Add a new `RenderedFile` struct, which specifies the template, and its path to [the `files` module](https://github.com/jhugman/uniffi-bindgen-react-native/blob/e7f85c616bf6985070081ec47f0b2b268890cc7d/crates/ubrn_cli/src/codegen/mod.rs#L141-L298) in [`codegen/mod.rs`][codegen/mod.rs].
1. Add an entry to [the list of generated files in this book](../reference/turbo-module-files.md).

The [template context](https://github.com/jhugman/uniffi-bindgen-react-native/blob/e7f85c616bf6985070081ec47f0b2b268890cc7d/crates/ubrn_cli/src/codegen/mod.rs#L55-L59) will have quite a lot of useful information data-structures about the project; the most prominent:

- [`ModuleMetadata`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/metadata.rs), which is generated from the `lib.a` file from the uniffi contents of the Rust library.
- [`ProjectConfig`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_cli/src/config/mod.rs) which is the in-memory representation of the [YAML configuration file](../reference/config-yaml.md).
- [`CrateMetadata`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_common/src/rust_crate.rs) which is data about the crate derived from `cargo metadata`.
