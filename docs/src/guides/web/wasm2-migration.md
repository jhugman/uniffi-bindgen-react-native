# Moving from `web` to `wasm2`

Both flavors run your crate as WebAssembly. The [`web` flavor](getting-started.md) gets there by generating a Rust crate of `#[wasm_bindgen]` wrappers around your library and building that; [`wasm2`](../../reference/wasm2/overview.md) builds your library for `wasm32-unknown-unknown` and drives it from a runtime player, so there is no second crate and no per-function glue.

If you already have a working `web` build, this page is the diff.

```admonish note
You do not have to choose. The two flavors read different sections of the configuration file and can generate into different directories, so you can stand `wasm2` up beside your existing `web` build, compare them, and delete the loser. The last section shows how.
```

## What moves into your crate

The `web` flavor puts the wasm-specific requirements in the crate it generates for you, so your own `Cargo.toml` never sees them. `wasm2` loads your crate directly, so they move to you.

```diff
  [lib]
- crate-type = ["lib"]
+ crate-type = ["lib", "cdylib"]

+ [target.'cfg(target_arch = "wasm32")'.dependencies]
+ uniffi-runtime-wasm = "0.31.0-3"
+ uniffi_core = { version = "0.31", features = ["wasm-unstable-single-threaded"] }
```

```diff
  // src/lib.rs
+ #[cfg(target_arch = "wasm32")]
+ extern crate uniffi_runtime_wasm as _;
```

Nothing here is new to your project — the generated wasm crate already depended on `uniffi-runtime-javascript` with its `wasm32` feature, which is what enabled `wasm-unstable-single-threaded` on your behalf. The dependency has changed name and moved one crate closer to you.

The `extern crate` line is the one with no counterpart. [`uniffi-runtime-wasm`](https://crates.io/crates/uniffi-runtime-wasm) exports the allocator and panic hook the player calls, and since nothing in your code references it, the linker would otherwise drop it.

```admonish warning
`ubrn build wasm2` checks the three manifest requirements and names whichever is missing. It cannot check the `extern crate` line. Leaving it out fails later, when opening the module reports `required export "__ubrn_alloc" not found in wasm module`.
```

## What leaves `ubrn.config.yaml`

The whole `web` section goes, and in the common case nothing replaces it:

```diff
- web:
-   manifestPath: rust_modules/wasm/Cargo.toml
-   ts: src/generated/web
-   entrypoint: src/index.web.ts
```

`wasm2` puts its bindings wherever [`bindings`/`ts`](../../reference/config-yaml.md#bindings) says, so a project that was happy with one output directory needs no section at all. Add a [`wasm2` section](../../reference/config-yaml.md#wasm2) only when you want a different directory, or non-default cargo features.

Most of the [`web` section](../../reference/config-yaml.md#web) described the crate that no longer exists. This is what happens to each key:

| `web` key | Under `wasm2` |
| --------- | ------------- |
| `ts` / `tsBindings` | same name, same meaning, and now where the `.wasm` is staged too — but only needed to override `bindings`/`ts` |
| `features`, `defaultFeatures` | same names, applied only to your crate; there is no second manifest to copy them into |
| `cargoExtras` | same |
| `manifestPath`, `wasmCrateName`, `workspace` | gone; no crate is generated, so there is nothing to name or place |
| `manifestPatchFile` | gone. It existed to patch the generated manifest — now you edit your own |
| `runtimeVersion` | gone; the runtime is a dependency you declare |
| `target`, `wasmBindgenExtras` | gone; `wasm-bindgen` is no longer invoked as a command |
| `entrypoint` | gone; there is no generated entrypoint to place, which is what the next section is about |

## What to add to `package.json`

Three things, one of which is easy to miss.

```diff
   "scripts": {
-    "ubrn:web": "ubrn build web",
+    "ubrn:web": "ubrn build wasm2",
   },
-  "browser": "src/index.web.ts",
+  "browser": "src/generated/index.ts",
   "dependencies": {
     "@ubjs/core": "^0.31.0-3",
+    "@ubjs/wasm": "^0.31.0-3"
   }
```

`ubrn build wasm2` generates by default, so there is no `--and-generate` to add.

The `browser` field is the one to watch. Under `web` it pointed at a file `ubrn` generated for you; `ubrn build wasm2` writes only the bindings, so it has to point at something that still exists. The generated `src/generated/index.ts` re-exports every namespace and is a complete entrypoint — but it leaves naming the `.wasm` to the caller, which is what the next section covers.

```admonish warning title="`generate all` still writes the web entrypoint"
`ubrn generate all --flavor wasm2` writes `src/index.web.ts` and the wasm crate under `rust_modules/` anyway: `--flavor` chooses the bindings generator, not the project files. The file it writes imports `generated/wasm-bindgen/index.js`, which `wasm2` does not produce, so it is broken on arrival.

`ubrn build wasm2` is scoped to this flavor and does not do that. If something in your pipeline calls `generate all`, exclude the stale files:

    noOverwrite:
      - src/index.web.ts
```

## What changes in the app

Under `web`, `ubrn` generates `src/index.web.ts` for you, and that file names the `.wasm` itself — which is why `uniffiInitAsync()` takes no arguments.

`ubrn build wasm2` writes no project entrypoint, because the bindgen already writes one: `src/generated/index.ts` re-exports every namespace and exports a `uniffiInitAsync` that takes the module. Naming the asset moves to the caller, since a bundler rewrites that name as it copies the file and only your host knows how.

Pointing `browser` at that generated file is the shortest migration, and moves the asset name into your app. Writing your own `src/index.web.ts` keeps it out of the app, at the cost of a file — the same path the `web` flavor used, except that now you own it:

```typescript
// src/index.web.ts
import { uniffiInitAsync as initBindings } from "./generated";
export * from "./generated";

export function uniffiInitAsync() {
    return initBindings(new URL("./generated/my_crate.wasm", import.meta.url));
}
```

Keeping the same exported name means the app that consumed the `web` build needs no change at all:

```js
import { uniffiInitAsync } from "my-rust-lib";

uniffiInitAsync().then(() => {
    AppRegistry.registerComponent(appName, () => App);
});
```

Under Metro, `new URL(...)` is not how assets resolve. Use the asset registry instead, as the [web tutorial](getting-started.md#teaching-metro-about-wasm-files) already has you configure:

```typescript
import { Asset } from "expo-asset";

export async function uniffiInitAsync() {
    const asset = Asset.fromModule(require("./generated/my_crate.wasm"));
    await asset.downloadAsync();
    return initBindings(asset.uri);
}
```

Either way, remember to point `browser` at whichever file you chose.

## What you can delete

- **The generated wasm crate**, wherever `web.manifestPath` pointed — usually `rust_modules/wasm/`. Nothing generates or reads it now.
- **The `wasm-bindgen` output directory** under your bindings, holding `index.js`, `index_bg.wasm` and their `.d.ts` files. `wasm2` stages a single `.wasm` beside the bindings instead.
- **`cargo install wasm-bindgen-cli`** from your setup instructions and CI. If your crate's dependencies reach wasm-bindgen, staging runs the rewrite in-process; there is no command to install.
- **`console_error_panic_hook`**, if you added it to see panics. The player installs a panic hook while opening the module, and prints `[Rust panic] <message>` with the JavaScript stack.
- **Any `noOverwrite` globs** covering the generated web crate — nothing generates it now. Keep, or add, one for `src/index.web.ts` if you wrote your own and anything in your pipeline still calls `generate all`.
- **The COEP and COOP headers** in `metro.config.js`, if you added them only for the `web` flavor. Those exist for `SharedArrayBuffer`, which the player does not use. Keep `assetExts.push('wasm')`, which `wasm2` still needs.

On the npm side, add the player and keep the shared runtime:

```sh
yarn add @ubjs/wasm @ubjs/core
```

## Running both while you migrate

Give each flavor its own output directory and its own script, and nothing collides:

```yaml
web:
  ts: src/generated/web

wasm2:
  ts: src/generated/web2
```

```diff
   "scripts": {
     "ubrn:web": "ubrn build web",
+    "ubrn:wasm2": "ubrn build wasm2",
```

The crate changes in the first section are additive — a `cdylib` alongside your `lib`, and dependencies behind `cfg(target_arch = "wasm32")` — so the `web` build keeps working while you try the other. Point `src/index.web.ts` at one directory or the other to switch.

Once you are happy, delete the `web` section, the generated crate, and the losing directory.

## What does not change

Your Rust API, the generated TypeScript API, and every line of app code that calls it. The bindings `wasm2` generates for a namespace are the same bindings the other flavors generate — only the file beneath them, and the way it is loaded, is different.

## Next

- [`wasm2` reference](../../reference/wasm2/reference.md) — the commands, the config, and what each error means.
- [`wasm2` cookbook](../../reference/wasm2/cookbook.md) — bundler wiring, custom entrypoints, workers, shrinking the module.
