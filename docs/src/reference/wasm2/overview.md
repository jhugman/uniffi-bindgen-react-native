# WebAssembly (`wasm2`) support

As of `0.31.0-3`, `uniffi-bindgen-react-native` can generate TypeScript bindings that run your Rust crate as a WebAssembly module, from a single `cargo build`, with no generated Rust shim crate and no per-crate JavaScript glue.

This is the `wasm2` flavor. It sits alongside the existing [`web` flavor](../../guides/web/getting-started.md), which builds a wasm-bindgen crate around your library; the two do the same job by different means, and both are supported.

```admonish warning
Hermes has no `WebAssembly`, so `wasm2` does not run in a standard React Native app — use the JSI target there. Metro is still supported as a *web* bundler, for Expo Web and React Native Web.
```

## How it works

`wasm2` has three pieces:

- **The bindgen** compiles nothing and reads everything. UniFFI metadata is embedded in your crate as `UNIFFI_META_*` symbols, and on `wasm32-unknown-unknown` those become exported globals pointing into the module's data segments. The bindgen reads them straight out of the `.wasm`, so the second, native-only build that other WASM tooling needs is not needed here.
- **The runtime**, published to npm as [`@ubjs/wasm`][wasm-npm], is a *player*: it reads a table of FFI signatures the bindgen emits beside your bindings, and calls the module through it.
- **The helper crate**, published to crates.io as [`uniffi-runtime-wasm`][wasm-crate], exports the two things the player needs and JavaScript cannot supply — an allocator for linear-memory scratch space, and a panic hook.

Because UniFFI uses a small, fixed set of FFI types, the signature table is data and the player is the same code for every library. That is the whole idea: the [`web` flavor](../../guides/web/getting-started.md) generates a Rust wrapper per FFI function into a shim crate and builds it; `wasm2` generates a table instead.

The TypeScript runtime helpers — FFI converters, the `RustBuffer` type, and so on — are shared with the other targets and are published as [`@ubjs/core`][core-npm].

## What your crate needs

Three lines in `Cargo.toml`, and one in `lib.rs`.

```toml
[lib]
crate-type = ["lib", "cdylib"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
uniffi-runtime-wasm = "0.31.0-3"
uniffi_core = { version = "0.31", features = ["wasm-unstable-single-threaded"] }
```

```rust
#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;
```

The player instantiates a WebAssembly module, so the crate has to link one. `uniffi-runtime-wasm` exports nothing you call, so nothing references it, so the linker would drop it — the `extern crate` line forces the link. Wasm is single-threaded, so without `wasm-unstable-single-threaded` UniFFI demands `Send + Sync` on every exported object and the crate will not compile for wasm32.

`ubrn build wasm2` checks all three manifest requirements before it starts, and names whichever is missing. The `extern crate` line it cannot check; see [the reference](reference.md#the-one-line-of-rust) for what a missing one looks like.

Putting the dependencies behind `cfg(target_arch = "wasm32")` keeps them off your native builds, so the same crate still serves the JSI and Node.js targets unchanged.

## Building

```sh
rustup target add wasm32-unknown-unknown
npm install @ubjs/wasm @ubjs/core
ubrn build wasm2 --config ubrn.config.yaml --release
```

That is one command doing three things: `cargo build --lib --target wasm32-unknown-unknown`, generate the TypeScript from the module cargo just wrote, and stage a copy of that module beside the TypeScript.

```
src/generated/
    receipts.ts             your API, in TypeScript
    receipts-ffi.ts         the signature table the player reads
    index.ts                the entrypoint
    receipt_scanner.wasm    your crate
```

## Loading the module

Instantiating a WebAssembly module is asynchronous everywhere, so the generated `index.ts` exports `uniffiInitAsync`. Await it once, before the first call.

```typescript
import { uniffiInitAsync, scanReceipt } from "./generated";

await uniffiInitAsync(new URL("./generated/receipt_scanner.wasm", import.meta.url));

const receipt = scanReceipt(imageBytes);   // sync call into Rust
const total = await receipt.totalAsync();  // Rust future, JS promise
```

You name the `.wasm` yourself, because a bundler rewrites that URL when it copies the file and only your host knows how. See [Loading the module](reference.md#loading-the-module) for the spelling each environment wants.

## How it compares

| Flavor | Host | Cargo builds | Generated per crate |
| ------ | ---- | ------------ | ------------------- |
| `jsi` | React Native / Hermes | one per target | TypeScript and C++ |
| `web` | Browser, Node.js | two (native, then wasm32) | TypeScript and a Rust shim crate |
| `wasm2` | Browser, Node.js | one (wasm32) | TypeScript |
| `napi` | Node.js | one (native) | TypeScript |

## Limitations

- **No Hermes**, and so no standard React Native. Use the JSI target.
- **No threads.** `wasm32-unknown-unknown` is single-threaded, and a Rust API that needs a host thread panics at the call rather than failing at the build.
- **No synchronous initialisation.** `uniffiInitAsync` must resolve before the first call into the module.

## See also

- [Moving from `web` to `wasm2`](../../guides/web/wasm2-migration.md) — the diff, if you already have a `web` build.
- [`wasm2` reference](reference.md) — every command, config key and runtime entry point.
- [`wasm2` cookbook](cookbook.md) — bundlers, custom entrypoints, strict CSP, workers, shrinking the module.
- [The `wasm2` player](../../internals/wasm2-player.md) — how the runtime works, for contributors.
- The [`@ubjs/wasm` README](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/runtimes/wasm) for the published package.

[core-npm]: https://www.npmjs.com/package/@ubjs/core
[wasm-npm]: https://www.npmjs.com/package/@ubjs/wasm
[wasm-crate]: https://crates.io/crates/uniffi-runtime-wasm
