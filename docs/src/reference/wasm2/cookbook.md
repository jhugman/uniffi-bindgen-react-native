# `wasm2` cookbook

Recipes that put several parts together. Each assumes your crate builds, the bindings generate, and `uniffiInitAsync` works — see the [reference](reference.md) if not.

The running example is still `receipt-scanner`: library `receipt_scanner`, namespace `receipts`. Names like `ensureReceipts` and `myWorkerPool` are yours, not the project's.

## One crate, two runtimes: React Native and the web

`wasm2` cannot run under Hermes, and JSI cannot run in a browser. A library that wants both generates both, into different directories, and lets the bundler choose.

```yaml
rust:
  directory: ./rust
  manifestPath: receipt-scanner/Cargo.toml

bindings:            # the JSI bindings land here
  ts: src/generated

wasm2:               # the wasm2 bindings land here
  ts: src/generated-web
```

```sh
ubrn build ios     --config ubrn.config.yaml --release --and-generate
ubrn build android --config ubrn.config.yaml --release --and-generate
ubrn build wasm2   --config ubrn.config.yaml --release
```

Two thin files pick a side:

```typescript
// src/index.native.ts
export * from "./generated";
```

```typescript
// src/index.web.ts
import { uniffiInitAsync } from "./generated-web";
export * from "./generated-web";

export const ready = uniffiInitAsync(
    new URL("./generated-web/receipt_scanner.wasm", import.meta.url),
);
```

```diff
   "main": "src/index.tsx",
+  "browser": "src/index.web.ts",
+  "react-native": "src/index.native.ts",
```

The asymmetry is real, and worth exposing rather than hiding: the web build has a `ready` promise and the native build does not. Consumers that must run on both `await ready` where it exists.

Your Rust source is untouched by any of this. The `wasm2` requirements sit behind `cfg(target_arch = "wasm32")`, so the iOS and Android builds never see them.

## Naming the `.wasm` for your bundler

`uniffiInitAsync` takes whatever your environment calls the file. The portable spelling works in Vite, webpack 5 and Node.js:

```typescript
await uniffiInitAsync(
    new URL("./generated/receipt_scanner.wasm", import.meta.url),
);
```

Vite also accepts an explicit URL import, which is clearer when the file is processed by a plugin:

```typescript
import wasmUrl from "./generated/receipt_scanner.wasm?url";
await uniffiInitAsync(wasmUrl);
```

Metro is different: `.wasm` goes through the asset registry, not the URL resolver, so the extension has to be registered — the same step the [web tutorial](../../guides/web/getting-started.md#teaching-metro-about-wasm-files) describes.

```js
// metro.config.js
const { getDefaultConfig } = require('expo/metro-config');

const config = getDefaultConfig(__dirname);
config.resolver.assetExts.push('wasm');

module.exports = config;
```

```typescript
import { Asset } from "expo-asset";

const asset = Asset.fromModule(require("./generated/receipt_scanner.wasm"));
await asset.downloadAsync();
await uniffiInitAsync(asset.uri);
```

```admonish note
If your crate reaches wasm-bindgen, `receipt_scanner_bg.js` sits beside the `.wasm` and the generated `index.ts` imports it. Your bundler follows that import on its own; you only have to make sure the two files travel together when you copy them by hand.
```

## Loading behind a route

A module used on one screen should not cost every user a download at boot. `uniffiInitAsync` is idempotent, so the guard is three lines.

```typescript
// src/receipts.ts
let opening: Promise<typeof import("./generated-web")> | undefined;

export function ensureReceipts() {
    opening ??= (async () => {
        const mod = await import("./generated-web");
        await mod.uniffiInitAsync(
            new URL("./generated-web/receipt_scanner.wasm", import.meta.url),
        );
        return mod;
    })();
    return opening;
}
```

```typescript
function ReceiptScreen({ image }: { image: Uint8Array }) {
    const [receipt, setReceipt] = useState<Receipt>();
    useEffect(() => {
        ensureReceipts().then(({ scanReceipt }) => setReceipt(scanReceipt(image)));
    }, [image]);
    // ...
}
```

The dynamic `import()` splits the bindings and the `.wasm` into their own chunk. `ensureReceipts` returns the same promise every time, so a second screen mounting mid-download joins the first download rather than starting another.

## Writing your own entrypoint

The generated `index.ts` covers the common case. Three exports let you replace it when you need to: `PLAYER_DEFINITIONS` and `setNativeModule` from `receipts-ffi.ts`, and `initialize` from the namespace's default export.

```typescript
import { openWasm, type WasmSource } from "@ubjs/wasm";
import { PLAYER_DEFINITIONS, setNativeModule } from "./generated/receipts-ffi";
import receipts from "./generated/receipts";

export async function openReceipts(source: WasmSource) {
    const mod = await openWasm(source);
    setNativeModule(mod.registerSync(PLAYER_DEFINITIONS));
    receipts.initialize();       // verifies checksums, installs vtables
    return mod;                  // keep it for `mod.memory`, `mod.exports`
}
```

`initialize()` is what catches a `.wasm` and a set of bindings that came from different builds: it compares a checksum per exported function, and names the first that disagrees.

If your crate reaches wasm-bindgen, hand the glue over too. This is exactly what the generated entrypoint does:

```typescript
import * as wasmBindgenGlue from "./generated/receipt_scanner_bg.js";

const mod = await openWasm(source, {
    resolveModule: async (name) =>
        name.endsWith("receipt_scanner_bg.js") ? wasmBindgenGlue : undefined,
});
```

```admonish warning title="One copy of the bindings drives one module"
`setNativeModule` sets a module-level binding inside the generated file. Two instances of the same crate need two realms — two workers, two iframes — not two calls.
```

## Running under a strict Content-Security-Policy

The player compiles a specialised dispatcher per exported function with `new Function`, which a page serving `script-src` without `'unsafe-eval'` forbids. It probes for this once, at registration, and falls back to an interpreted dispatcher with identical behaviour. Under a strict CSP everything works; calls carry a little more overhead.

Where you would rather not depend on a probe — testing the fallback, or pinning behaviour across environments — write the entrypoint above and say so:

```typescript
setNativeModule(mod.registerSync(PLAYER_DEFINITIONS, { disableJit: true }));
```

`new Function` is the only thing this affects. The callback path compiles small wasm modules of its own, but a browser gates *all* wasm compilation behind `'wasm-unsafe-eval'` — so a page that can load your module at all can compile trampolines too.

## Compile once, instantiate many

Compiling a `.wasm` is the expensive half; instantiating it is cheap. When you run the same crate in several workers, compile in one place and post the `WebAssembly.Module` — which is structured-cloneable — to each.

```typescript
// main thread
const compiled = await WebAssembly.compileStreaming(
    fetch("/assets/receipt_scanner.wasm"),
);
for (const worker of myWorkerPool) {
    worker.postMessage({ kind: "receipts/module", compiled });
}
```

```typescript
// worker
self.onmessage = async ({ data }) => {
    if (data.kind === "receipts/module") {
        const { uniffiInitAsync } = await import("./generated-web");
        await uniffiInitAsync(data.compiled);   // a WebAssembly.Module, no fetch
    }
};
```

Each worker gets its own linear memory, its own Rust-side state, and its own copy of the bindings. Nothing is shared, which is the point: wasm here is single-threaded, and a worker is how you keep a long Rust call off the main thread.

## Moving large byte payloads

Records, strings and `Vec<u8>` cross the boundary as a serialised buffer. The generated call sites ask the player for wasm memory up front and write the payload straight into it, so a `Vec<u8>` argument costs one copy — the write — rather than a JavaScript array followed by a copy into wasm. Returns work the same way in reverse: the player hands the generated code a view aliasing wasm memory, the converter reads it, and a `finally` frees the allocation even when the conversion throws.

Design your Rust API to take that path, and you get it for free:

```rust
#[uniffi::export]
pub fn scan_receipt(image: Vec<u8>) -> Result<Receipt, ScanError> { /* ... */ }

#[uniffi::export]
pub fn render_thumbnail(receipt: &Receipt) -> Vec<u8> { /* ... */ }
```

```typescript
const thumbnail = renderThumbnail(receipt);  // a Uint8Array, one copy out
```

The single rule, and the only way to get this wrong, is holding one of those views. They alias wasm linear memory, and growing that memory detaches them. If you write your own entrypoint and call `rustbuffer_alloc` directly, free the view before anything else can allocate:

```typescript
const nativeModule = mod.registerSync(PLAYER_DEFINITIONS);
setNativeModule(nativeModule);

const view = nativeModule.rustbuffer_alloc(4096);
myEncoder.writeInto(view);
// ... hand it to a call, or free it. Do not keep it across another call.
```

```admonish warning
Held across a growth, such a view reports a `byteLength` of zero, and freeing it throws `rustbuffer_free: view is detached`. The error is deliberate — silently leaking would be worse.
```

## Testing the bindings with `node --test`

Test scripts read better when they import the API and nothing else. Put the loading in a preload module: Node.js runs `--import` modules to completion, including their top-level `await`, before the entry module.

```typescript
// test/bootstrap.ts
import { uniffiInitAsync } from "../src/generated-web/index.js";
await uniffiInitAsync(
    new URL("../src/generated-web/receipt_scanner.wasm", import.meta.url),
);
```

```typescript
// test/receipts.test.ts — no loading, no awaiting init
import { test } from "node:test";
import assert from "node:assert";
import { scanReceipt } from "../src/generated-web/index.js";

test("reads the merchant off a receipt", () => {
    assert.equal(scanReceipt(myFixtureImage).merchant, "Grocer");
});
```

```sh
node --import ./test/bootstrap.ts --test test/
```

The same script then runs unchanged against the JSI or Node.js bindings, whose loading happens elsewhere. That is how this project's own fixture suite runs one set of test scripts across four flavors.

## Making the module smaller

`ubrn build wasm2` does not strip or optimise your crate. It could — but the keep-list would be a heuristic, and your `cdylib` may export symbols for a consumer the command line cannot see. Shrinking is yours, and worth doing: a release build carries far more than the calls you make.

Start with the profile:

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = "debuginfo"
```

Then run `wasm-opt` over the staged module:

```sh
ubrn build wasm2 --config ubrn.config.yaml --release
wasm-opt -Oz --enable-bulk-memory \
    src/generated-web/receipt_scanner.wasm \
    -o src/generated-web/receipt_scanner.wasm
```

Enable whatever wasm features your toolchain emitted; `wasm-opt` names the one it choked on when you have missed one. Run the module once afterwards — a wrong flag shows up as a failed instantiation, not as a wrong answer.

```admonish warning title="Two things not to strip"
The player needs `memory`, `__indirect_function_table` and the `__ubrn_*` exports; removing any of them breaks loading rather than saving much. Dropping the wasm name section saves a real share of a release module, and turns every Rust panic into hex.
```

## Reading a Rust panic

The player installs a panic hook while opening the module and points it at a JavaScript function, so a panic inside Rust prints:

```
[Rust panic] called `Option::unwrap()` on a `None` value
Error
    at scanReceipt (receipts.ts:118:12)
    ...
```

The Rust message comes from the hook; the stack below it is the JavaScript stack at the moment of the panic, which is usually the more useful half, because it names the call you made.

There are two ways to see nothing at all. If the runtime crate is not linked — the [`extern crate` line](reference.md#the-one-line-of-rust) — the module has no hook to install, and opening it fails earlier with `required export "__ubrn_alloc" not found`. And anything that panics in the module's start section fires before the player has installed the hook, so it lands as an instantiation failure instead.

```admonish warning
After a panic, that module's Rust state is not trustworthy: a lock may still be held, an allocation half-made. Treat it as fatal for the instance and open a fresh one, which in a worker means restarting the worker.
```
