# Summary

[Introduction](README.md)
# Getting started for React Native

- [Before you start](guides/rn/pre-installation.md)
- [Step by step: Make your first library project](guides/rn/getting-started.md)
- [Troubleshooting](guides/rn/troubleshooting.md)

# Getting started for the Web

- [Before you start](guides/web/pre-installation.md)
- [Step by step: Make your first library project](guides/web/getting-started.md)
- [Troubleshooting](guides/web/troubleshooting.md)

# Guides

- [Publishing your library project](guides/publishing.md)
- [Working with multiple crates in one library](guides/megazords.md)

# Mapping Rust on to Typescript

- [Common types](idioms/common-types.md)
- [Objects: Objects with methods](idioms/objects.md)
  - [Garbage Collection and the Drop trait](idioms/gc.md)
- [Records: Objects without methods](idioms/records.md)
- [Enums and Tagged Unions](idioms/enums.md)
- [Errors](idioms/errors.md)
- [Callback interfaces](idioms/callback-interfaces.md)
- [Promises/Futures](idioms/promises.md)
- [Async Callback interfaces](idioms/async-callbacks.md)
- [Option and Result](idioms/option-result.md)
- [Threading](idioms/threading.md)

# Contributing

- [Local development](contributing/local-development.md)
- [Turbo-module and WASM crate templates: Adding or changing](contributing/changing-turbo-module-templates.md)
- [Documentation: Contributing or reviewing](contributing/documentation.md)
- [Typescript, C++, WASM crate bindings templates: Changing](contributing/changing-bindings-templates.md)
- [Unit Testing the command line](contributing/testing_the_command_line.md)
- [Cutting a Release](./contributing/cutting-a-release.md)

# Reference

- [`ubrn` Command Line](reference/commandline.md)
- [`ubrn.config.yaml`](reference/config-yaml.md)
- [`uniffi.toml`](reference/uniffi-toml.md)
- [Generating a Turbo Module](reference/turbo-module-files.md)
- [Reserved words](reference/reserved-words.md)
- [Potential collisions](reference/potential-collisions.md)

# Internals

- [Lifting and lowering](./internals/lifting-and-lowering.md)
- [NativeModule.ts and Codegen](./internals/rn-codegen.md)
