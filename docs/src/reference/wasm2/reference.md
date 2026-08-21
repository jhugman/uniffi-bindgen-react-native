# `wasm2` reference

Everything the `wasm2` flavor does, in the order you meet it: prepare the crate, build it, load it, call it. This page assumes you have read the [overview](overview.md).

The examples come from a made-up crate called `receipt-scanner`, whose library name is `receipt_scanner` and whose UniFFI namespace is `receipts`. Names like `ReceiptStore` and `myScanner` belong to that crate, not to `uniffi-bindgen-react-native`.

## Prerequisites

| Piece | Comes from | What it does |
| ----- | ---------- | ------------ |
| `uniffi-bindgen-react-native` | cargo or npm | the `ubrn` command line: builds, generates, stages |
| [`uniffi-runtime-wasm`](https://crates.io/crates/uniffi-runtime-wasm) | crates.io | the allocator and panic hook, exported from your `cdylib` |
| [`@ubjs/wasm`](https://www.npmjs.com/package/@ubjs/wasm) | npm | the player: reads the signature table, calls the module |
| [`@ubjs/core`](https://www.npmjs.com/package/@ubjs/core) | npm | the shared TypeScript runtime, a peer dependency of `@ubjs/wasm` |

```sh
rustup target add wasm32-unknown-unknown
npm install @ubjs/wasm @ubjs/core
```

The player runs wherever `WebAssembly` does: modern browsers, Node.js 18 or later, Bun, and web bundlers.

## Preparing the crate

### The manifest

```toml
# receipt-scanner/Cargo.toml
[lib]
crate-type = ["lib", "cdylib"]

[dependencies]
uniffi = "0.31"

[target.'cfg(target_arch = "wasm32")'.dependencies]
uniffi-runtime-wasm = "0.31.0-3"
uniffi_core = { version = "0.31", features = ["wasm-unstable-single-threaded"] }
```

Each line earns its place. `cdylib` is what produces a WebAssembly module at all. `uniffi-runtime-wasm` supplies `__ubrn_alloc`, `__ubrn_free` and a panic hook, all of which the player calls and none of which JavaScript can provide. `wasm-unstable-single-threaded` drops UniFFI's `Send + Sync` requirement on exported objects, which wasm32 cannot satisfy; spelling the feature on `uniffi` rather than `uniffi_core` works too, since `uniffi` re-exports it.

The `[target.'cfg(target_arch = "wasm32")']` block keeps all of this off your native builds, so the same crate still serves the JSI and Node.js targets.

### The one line of Rust

```rust
// receipt-scanner/src/lib.rs
#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;
```

`uniffi-runtime-wasm` exports nothing you call, so nothing in your code references it, so the linker drops it — and the module reaches the player without an allocator. The `extern crate` line forces the link.

```admonish warning title="This is the one thing the build cannot check"
`ubrn build wasm2` reads your `Cargo.toml`, so it catches a missing dependency. It cannot see whether you referenced it. Omitting the line surfaces later, when opening the module fails with `required export "__ubrn_alloc" not found in wasm module`.
```

## `ubrn build wasm2`

```sh
ubrn build wasm2 --config ubrn.config.yaml --release
```

One command, three steps: `cargo build --lib --target wasm32-unknown-unknown`, generate the TypeScript from the module cargo has just written, and stage a copy of that module beside the TypeScript, adding the exports the player needs.

Before any of that, it reads your manifest and rejects a crate the player cannot load, naming what is missing. Letting `cargo` fail would say less, and letting it succeed would defer the failure to your first call.

```sh
Usage: uniffi-bindgen-react-native build wasm2 [OPTIONS]

Options:
      --config <CONFIG>
          The configuration file for this project

      --no-generate
          Opts out of generating the bindings and wasm-crate

      --no-wasm-build
          Opts out of running cargo build for wasm32-unknown-unknown

  -r, --release
          Build a release build

  -p, --profile <PROFILE>
          Use a specific build profile

  -g, --and-generate
          Optionally generate the bindings and turbo-module code for the crate
```

```admonish note
`ubrn build wasm2` generates by default, so the `-g` / `--and-generate` flag that the other platforms need does nothing here. Use `--no-wasm-build` — not the shared `--no-cargo` flag — to reuse a module already in `target/`.
```

The `wasm2` section of the [configuration file](../config-yaml.md#wasm2) controls where the TypeScript lands and how the crate is built. A minimal one:

```yaml
rust:
  directory: ./rust
  manifestPath: receipt-scanner/Cargo.toml

wasm2:
  ts: src/generated
```

## `ubrn generate wasm2`

Generating without a configuration file takes the `.wasm` and an output directory.

```sh
ubrn generate wasm2 bindings \
  --library target/wasm32-unknown-unknown/release/receipt_scanner.wasm \
  --ts-dir src/generated
```

| Option | Description |
| ------ | ----------- |
| `--library` | Treat `<SOURCE>` as a library and extract the UniFFI definitions from it. |
| `--ts-dir <DIR>` | The directory the generated TypeScript is written to. |
| `--config <CONFIG>` | The location of the [`uniffi.toml` file](../uniffi-toml.md). |
| `--no-format` | Skip formatting the generated code with `prettier`, which is run by default. |

This writes TypeScript and nothing else. Copying the module beside it is still yours to do, which is what `ubrn build wasm2` would have done for you.

```admonish warning
Point `--library` at **cargo's own output**, not at a staged copy. Staging may rewrite the module, and generation reads UniFFI metadata out of it. A staged module reports `no UNIFFI_META_* exports found in WASM file`.
```

The `generate wasm2 wasm-crate` subcommand exists for symmetry with the other flavors and renders no files: `wasm2` needs no shim crate and no project entrypoint, because the bindgen's own `index.ts` is the entrypoint.

```admonish warning title="`generate all --flavor wasm2` is not the same command"
`--flavor` on `generate all` chooses the bindings generator, not the project files. It still writes the JSI turbo-module scaffolding, the `web` flavor's wasm crate under `rust_modules/`, and a `src/index.web.ts` that imports files `wasm2` does not produce.

`ubrn build wasm2` and `ubrn generate wasm2 bindings` are both scoped to this flavor. Prefer those, and use [`noOverwrite`](../config-yaml.md#nooverwrite) to fence off anything a `generate all` in your pipeline would write.
```

## What gets generated

```
src/generated/
    receipts.ts             the API: your functions, objects, records, errors
    receipts-ffi.ts         the signature table, and TypeScript types for it
    index.ts                the entrypoint: opens the module, registers namespaces
    receipt_scanner.wasm    your crate, staged
    receipt_scanner_bg.js   wasm-bindgen glue, only if your crate needs it
```

`receipts.ts` is what you import, and is the same code the JSI and Node.js targets generate.

`receipts-ffi.ts` holds the signature table — every FFI export, described by argument and return type — and exports it as `PLAYER_DEFINITIONS`. It imports nothing environment-specific, so it bundles for Node.js, browsers and React Native Web alike.

`index.ts` is the entrypoint, and the only generated file that opens the module.

## Loading the module

```typescript
import { uniffiInitAsync } from "./generated";

await uniffiInitAsync(source);
```

`uniffiInitAsync` is idempotent: a second call returns the first call's promise. Call it once, at startup, before anything touches the API.

### Naming the `.wasm`

`source` is the module, however your environment names it. There is no default, because naming an asset is the one thing only the host knows — a bundler rewrites the URL as it copies the file.

| Environment | `source` |
| ----------- | -------- |
| Vite, webpack 5, Node.js ESM | `new URL("./generated/receipt_scanner.wasm", import.meta.url)` |
| Vite, explicitly | `import url from "./generated/receipt_scanner.wasm?url"` |
| Browser, hand-rolled | `fetch("/assets/receipt_scanner.wasm")` |
| Anywhere, from bytes | a `Uint8Array` or an `ArrayBuffer` |
| Already compiled | a `WebAssembly.Module` |

`new URL(..., import.meta.url)` is the portable spelling: Vite, webpack 5 and Node.js all rewrite or resolve it. The browser build also accepts a bare `Promise<Response>`, so a `fetch` can go straight in without an intervening `await`:

```typescript
await uniffiInitAsync(fetch("/assets/receipt_scanner.wasm"));
```

Under Metro — Expo Web, React Native Web — `.wasm` goes through the asset registry rather than the URL resolver, exactly as it does for the [`web` flavor](../../guides/web/getting-started.md#teaching-metro-about-wasm-files). Register the extension in `metro.config.js`, then resolve the asset with `Asset.fromModule(...).uri`.

### Calling before the module is open

Every generated call goes through a getter that throws with your crate's name:

```
receipt-scanner: wasm module not initialised. Await `uniffiInitAsync(...)`
from the generated entrypoint before calling into this module.
```

If your app cannot await at import time, hold the promise and await it at the first call site; the cookbook shows [a lazy entrypoint](cookbook.md#loading-behind-a-route).

## Calling Rust

Once initialised, the API behaves as it does under every other target. The mapping from Rust to TypeScript is [documented separately](../../idioms/common-types.md); what follows is only how it looks under `wasm2`.

### Functions, records and errors

```rust
#[derive(uniffi::Record)]
pub struct Receipt { pub merchant: String, pub total_pence: u32 }

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ScanError {
    #[error("the image was unreadable")]
    Unreadable,
}

#[uniffi::export]
pub fn scan_receipt(image: Vec<u8>) -> Result<Receipt, ScanError> { /* ... */ }
```

```typescript
import { scanReceipt, ScanError } from "./generated";

try {
    const receipt = scanReceipt(imageBytes);
    console.log(receipt.merchant, receipt.totalPence);
} catch (e) {
    if (ScanError.instanceOf(e)) {
        // handle it
    }
}
```

Records, strings and byte arrays cross the boundary as a serialised buffer. The generated code asks the player for wasm memory up front and writes the payload straight into it, rather than building a JavaScript buffer and copying it in; return values come back as a view over wasm memory and are freed in a `finally`, so a throwing conversion still releases them.

### Objects

```rust
#[derive(uniffi::Object)]
pub struct ReceiptStore { /* ... */ }

#[uniffi::export]
impl ReceiptStore {
    #[uniffi::constructor]
    pub fn new() -> Self { /* ... */ }
    pub fn save(&self, receipt: Receipt) -> u64 { /* ... */ }
}
```

```typescript
const store = new ReceiptStore();
const id = store.save(receipt);
```

Objects are handles into a Rust-side registry, released through a `FinalizationRegistry` when the JavaScript object is collected — see [Garbage Collection and the Drop trait](../../idioms/gc.md). Where you need the drop to happen at a known moment, `store.uniffiDestroy()` does it now.

## Async

Rust futures become JavaScript promises, with nothing extra to configure.

```rust
#[uniffi::export]
impl ReceiptStore {
    pub async fn total_for_month(&self, month: u8) -> u32 { /* ... */ }
}
```

```typescript
const total = await store.totalForMonth(3);
```

The player registers the continuation as a wasm function and hands Rust its index in the module's function table; Rust calls it when the future can make progress. Each poll resolves one promise, and the loop runs until the future reports ready.

Cancellation rides on `AbortSignal` where the Rust API supports it:

```typescript
const controller = new AbortController();
const total = store.totalForMonth(3, { signal: controller.signal });
controller.abort();  // rejects with an AbortError
```

```admonish warning title="Futures that need a host thread"
Wasm32 has no threads. A hand-rolled `Future` that calls `std::thread::spawn` to wake itself panics on `wasm32-unknown-unknown`, and the panic leaves that module's Rust state untrustworthy. Futures produced by `async fn` and driven by UniFFI's own poll loop are fine.
```

## Callback interfaces

A [callback interface](../../idioms/callback-interfaces.md) is a trait Rust calls and TypeScript implements. Under `wasm2` this works exactly as it does elsewhere.

```rust
#[uniffi::export(callback_interface)]
pub trait LedgerObserver: Send + Sync {
    fn on_receipt(&self, receipt: Receipt);
}

#[uniffi::export]
pub fn watch_ledger(observer: Box<dyn LedgerObserver>) { /* ... */ }
```

```typescript
watchLedger({
    onReceipt(receipt) {
        console.log("saw", receipt.merchant);
    },
});
```

Rust can only call a wasm function, so the player installs a small trampoline in the module's function table for each method of the vtable, and routes the call back into your closure. The generated code builds each interface's vtable once, at module scope, with instance identity travelling as an argument — so a thousand observers cost the same table space as one.

[Async callback interfaces](../../idioms/async-callbacks.md) work the same way; Rust receives a foreign future and polls it, and your promise settling completes it.

## Crates that reach wasm-bindgen

Plenty of crates pull in `wasm-bindgen` on wasm32 — anything wanting a clock, a random number, or `fetch`. Such a build imports a placeholder namespace that only wasm-bindgen's own rewriter can resolve.

Staging notices this, runs the rewrite over your module, and leaves `receipt_scanner_bg.js` beside the `.wasm`. The generated `index.ts` imports that file statically:

```typescript
import * as wasmBindgenGlue from "./receipt_scanner_bg.js";
```

A static import keeps the glue in your bundler's graph. Fetching it at runtime would work too, and would be invisible to every bundler — so it is not what happens.

You need do nothing, but two things are worth knowing. The `.wasm` and the `_bg.js` beside it are a matched pair, so copy both or neither. And the rewriter must match the `wasm-bindgen` crate your module was built against; a version skew fails the build with a schema-version error rather than misbehaving later.

## Several namespaces in one module

A crate that re-exports UniFFI types from its dependencies — a [megazord](../../guides/megazords.md) — produces one `.wasm` with several namespaces. The generated `index.ts` opens the module once and registers each namespace against it:

```typescript
import { uniffiInitAsync } from "./generated";
import { scanReceipt } from "./generated/receipts";
import { exportLedger } from "./generated/ledger";

await uniffiInitAsync(wasm);  // opens once, registers both
```

`index.ts` also re-exports every namespace, so importing `./generated` alone is enough when the names do not collide.

## The runtime API

You rarely touch this. It matters when you write your own entrypoint — see [Writing your own entrypoint](cookbook.md#writing-your-own-entrypoint).

`@ubjs/wasm` resolves to a browser or a Node.js build through the package's `exports` conditions.

```typescript
function openWasm(
    source: WasmSource,
    options?: { resolveModule?: ImportResolver },
): Promise<UniffiNativeModule>;
```

The browser build fetches a `URL` or a string and accepts a `Response` or a `Promise<Response>`; the Node.js build reads a `URL` or a path off disk. Both hand bytes, an `ArrayBuffer` or a `WebAssembly.Module` straight through.

`resolveModule` supplies the module's own imports, which is how the wasm-bindgen glue above is delivered. Anything it declines to satisfy is filled with a stub that throws when called, so an unwired import fails loudly instead of quietly doing nothing.

```typescript
class UniffiNativeModule {
    readonly memory: Memory;
    readonly exports: WebAssembly.Exports;
    registerSync(
        definitions: ModuleDefinitions,
        opts?: { disableJit?: boolean },
    ): NativeModuleInterface;
}
```

`registerSync` turns a namespace's `PLAYER_DEFINITIONS` into callable JavaScript, one function per FFI export, plus a `rustbuffer_alloc` and `rustbuffer_free` pair. It is synchronous and cheap, the module being already instantiated by the time you hold one of these.

`disableJit` forces the interpreted dispatcher. The player already falls back to it where `new Function` is unavailable, so the flag is for tests and for pinning behaviour deliberately.

The `@ubjs/wasm/core` subpath is the environment-neutral half: `FfiType`, `UniffiNativeModule` and the types. The generated `receipts-ffi.ts` imports only this, which is why it bundles anywhere.

## Errors you may meet

| Message | Cause |
| ------- | ----- |
| `does not build a cdylib` | `[lib] crate-type` is missing `cdylib` |
| `does not depend on uniffi-runtime-wasm` | the manifest is missing the runtime crate |
| `resolves uniffi_core without the wasm-unstable-single-threaded feature` | the feature is not enabled for wasm32 |
| `required export "__ubrn_alloc" not found in wasm module` | the [`extern crate` line](#the-one-line-of-rust) is missing |
| `no UNIFFI_META_* exports found in WASM file` | generating from a staged module instead of cargo's output |
| `wasm module not initialised` | a call landed before `uniffiInitAsync` resolved |
| `register: wasm export "<name>" not found` | the bindings and the `.wasm` came from different builds |
| `host wasm does not export __indirect_function_table` | the module was never staged, so it lacks the growable table export |
| `wasm import <mod>.<name> is a stub` | the module wants glue nothing supplied, usually a missing `_bg.js` |
| `rustbuffer_free: view is detached` | a buffer view was held across an operation that grew wasm memory |
| `No wasm module at <path>` | `--no-wasm-build`, with nothing built yet |

## What is not supported

- **Hermes**, and so standard React Native. Use the JSI target.
- **Threads**, and any Rust API that requires one.
- **Synchronous initialisation.** Instantiating wasm is asynchronous everywhere.
- **Synchronous re-entry of one export.** A callback that calls back into the same Rust function while the outer call is still on the stack would corrupt that call. UniFFI's callback model does not generate this shape; see [the player's design](../../internals/wasm2-player.md#trade-offs) for why the dispatcher is built this way.
