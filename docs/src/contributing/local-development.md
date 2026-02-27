# Local development  of `uniffi-bindgen-react-native`

## Pre-installation

This guide is in addition to the Pre-installation guides for [React Native](../guides/rn/pre-installation.md) and [WASM](../guides/web/pre-installation.md).

```sh
git clone https://github.com/jhugman/uniffi-bindgen-react-native
cd uniffi-bindgen-react-native
```

Now you need to run the bootstrap xtask:

```sh
cargo xtask bootstrap
```

The first time you run this will take some time: it clones the main branch of [`facebook/hermes`](https://github.com/facebook/hermes) and builds it.

By default, it checks out the `main` branch, but this can be customized:

```sh
cargo xtask bootstrap hermes --branch rn/0.76-stable
```

It also builds the `cpp/test-harness` which is the Javascript runtime which can accept `.so` files written in C++ and Rust.

You can force a re-setup with:

```sh
cargo xtask bootstrap --force
```

Tests to see if a bootstrap step can be skipped is fairly rudimentary: mostly just the existence of a directory, so switching to a new branch of `hermes` would be done:

```sh
cargo xtask clean
cargo xtask bootstrap hermes --branch rn/0.76-stable
cargo xtask bootstrap
```

## Running tests

Most of the testing for `uniffi-bindgen-react-native` is done in the `fixtures` directory by testing the generated Typescript and C++ against a Rust crate.

These can be run with:

```sh
./scripts/run-tests.sh
```

One or more fixtures may be run using the `-f` flag.

```sh
./scripts/run-tests.sh -f chronological -f arithmetic
```

This, in turn, uses the `run` xtask.

The `run-tests.sh` script also runs the Typescript-only tests in the `typescript/tests` directory.

These have been useful to prototype generated Typescript before moving them into templates.

`run-tests.sh` also runs the same fixtures with WASM:

```sh
./scripts/run-tests.sh --flavor wasm
```

### Running rust only unit tests

Rust unit tests are encouraged! They can be run as usual with:

```sh
cargo test
```

## Formatting and linting

Pre-commit, you should ensure that the code is formatted.

The `fmt` xtask will run `cargo fmt`, `cargo clippy` on the Rust, `prettier` on Typescript and `clang-tidy` on C++ files.

```sh
cargo xtask fmt
```

Running with the `--check` does not change the files, just finishes abnormally if any of the formatters find something it would like changed.

```sh
cargo xtask fmt --check
```

## Before pushing a PR

Ensure that the following all run cleanly:

```sh
cargo xtask fmt
./scripts/run-tests.sh --flavor jsi
./scripts/run-tests.sh --flavor wasm
```
