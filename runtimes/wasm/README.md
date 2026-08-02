# @ubjs/wasm

The WebAssembly player runtime for [`uniffi-bindgen-react-native`][bindgen]
generated bindings. Imported by generated TypeScript code; not intended for
direct use.

[bindgen]: https://github.com/jhugman/uniffi-bindgen-react-native

## What it does

The player reads a table of FFI signatures the bindgen emits alongside your
bindings, and calls the `.wasm` through it. Nothing per-crate is generated in
C++ or JavaScript — one runtime drives every module.

## Compatibility

Runs anywhere `WebAssembly` does: modern browsers, Node ≥18, Bun, and web
bundlers. The `exports` map picks the browser or node build through the
`browser` and `node` conditions.

Hermes has no `WebAssembly`, so this does not run in a standard React Native
app. Use the JSI flavor there. Metro is still supported as a *web* bundler
(Expo Web, React Native Web).

## Install

You should not install this directly. Generated bindings import it, and list
`@ubjs/core` as a peer requirement.

```bash
npm install @ubjs/wasm @ubjs/core
```

## Loading a module

Generated bindings export `uniffiInitAsync`, which takes the `.wasm` however
your environment names it — a URL, a path, a `Response`, or bytes. Naming the
asset is the one thing only the host knows, so there is no default.

```ts
import { uniffiInitAsync } from './generated';

import wasm from './generated/my_crate.wasm';                    // bundler
const wasm = new URL('./generated/my_crate.wasm', import.meta.url); // node

await uniffiInitAsync(wasm);
```

Under Metro, `.wasm` goes through the asset registry rather than becoming a
URL; resolve it with `Asset.fromModule(...).uri`, or serve the file and pass
its URL.

## Rust side

Your crate builds a `cdylib` and depends on the [`uniffi-runtime-wasm`][crate]
crate, which supplies the allocator and panic hook the player calls into.
`ubrn build wasm2` checks both, and `uniffi_core` needs its
`wasm-unstable-single-threaded` feature.

[crate]: https://crates.io/crates/uniffi-runtime-wasm
