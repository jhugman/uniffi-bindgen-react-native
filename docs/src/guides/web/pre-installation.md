# Before you start

Better resources are available than this site for installing these dependencies.

Below are a list of the dependencies, and a non-comprehensive instructions on how to get them onto your system.

## Install Rust

If Rust isn't already installed on your system, you should install it as per the [rust-lang.org install instructions](https://www.rust-lang.org/tools/install).

This will add `cargo` and `rustup` to your path, which are the main entry points into Rust.

### Add the WASM specific targets

This command adds the backends for the Rust compiler to emit machine code for different Android architectures.

```sh
rustup target add \
    wasm32-unknown-unknown
```

### Install `wasm-bindgen`

> This cargo extension handles all the environment configuration needed for successfully building libraries for Android from a Rust codebase, with support for generating the correct jniLibs directory structure.

```sh
cargo install wasm-bindgen-cli
```

## Install nodejs

If `nodejs` isn't already installed on your system, you should install it as per the [nodejs.org install instructions](https://nodejs.org/en/download).

This guide and related documentation assumes `yarn` as a package manager.
