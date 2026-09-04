# Before you start

Better resources are available than this site for installing these dependencies.

Below are a list of the dependencies, and a non-comprehensive instructions on how to get them onto your system.

## Install Rust

If Rust isn't already installed on your system, you should install it as per the [rust-lang.org install instructions](https://www.rust-lang.org/tools/install).

This will add `cargo` and `rustup` to your path, which are the main entry points into Rust.

### Add the WASM specific target

This command adds the backend for the Rust compiler to emit WebAssembly.

```sh
rustup target add \
    wasm32-unknown-unknown
```

### Install `wasm-bindgen`

> This command rewrites a compiled `.wasm` so that JavaScript can call it, resolving the imports the `wasm-bindgen` crate leaves behind for it at compile time.

```sh
cargo install wasm-bindgen-cli
```

The rewriter takes only the version of the `wasm-bindgen` crate your module was built against, so once your `Cargo.lock` has settled on one, install that:

```sh
cargo install wasm-bindgen-cli --version 0.2.127  # whatever your lock says
```

`ubrn` pins no version of its own, and names the one it wants when the binary it finds disagrees. Set `UBRN_WASM_BINDGEN` to a path when the right binary cannot go on `PATH` — a machine building two projects can need two of them.

## Install nodejs

If `nodejs` isn't already installed on your system, you should install it as per the [nodejs.org install instructions](https://nodejs.org/en/download).

This guide and related documentation assumes `yarn` as a package manager.
