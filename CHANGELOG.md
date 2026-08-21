# Upcoming releases

[//]: # (## ✨ What's New ✨)
[//]: # (## 🦊 What's Changed)
[//]: # (## ⚠️ Breaking Changes)
[//]: # (**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/{{previous}}...{{current}})

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.31.0-5...main

---

# 0.31.0-5

## ✨ What's New ✨

### 🕸️ A new WASM flavour: `wasm2`

`uniffi-bindgen-react-native` gains a second WASM target, built on the same player architecture as the Node.js runtime shipped in 0.31.0-3. There is no shim crate to generate and no per-library glue to compile: `ubrn build wasm2` compiles your crate once for `wasm32-unknown-unknown`, and the bindgen reads the UniFFI metadata straight out of that same `.wasm`, so the second native-only cargo build disappears. The generated wrapper opens no module itself, which leaves it environment-neutral — the same bindings bundle for Node, browsers and React Native alike.

Getting started: your crate needs to be a `cdylib`, link the new [`uniffi-runtime-wasm`](https://crates.io/crates/uniffi-runtime-wasm) helper crate, and enable the `uniffi_core` feature that drops `Send + Sync` on exported objects. `ubrn build wasm2` checks all three up front rather than letting cargo fail later. The player runtime is published to npm as [`@ubjs/wasm`](https://www.npmjs.com/package/@ubjs/wasm).

- The player runtime, plus the `uniffi-runtime-wasm` helper crate consuming cdylibs link ([#418](https://github.com/jhugman/uniffi-bindgen-react-native/pull/418)).
- Post-link processing of the wasm module — staging, a growable function table for callback trampolines, and dead-export stripping — and the crate-manifest queries the build validates ([#424](https://github.com/jhugman/uniffi-bindgen-react-native/pull/424)).
- The `Wasm2` ABI flavour in the bindgen, exporting `PLAYER_DEFINITIONS` and `setNativeModule` from the per-module wrapper, with a generated `index.ts` doing the opening ([#425](https://github.com/jhugman/uniffi-bindgen-react-native/pull/425)).
- The `ubrn build wasm2` and `ubrn generate wasm2` subcommands, aliased `web2` ([#426](https://github.com/jhugman/uniffi-bindgen-react-native/pull/426)).
- The full fixture suite now runs against the flavour ([#427](https://github.com/jhugman/uniffi-bindgen-react-native/pull/427)) in CI ([#428](https://github.com/jhugman/uniffi-bindgen-react-native/pull/428)). As with the existing `wasm` flavour, the `futures` fixture is excluded: its `TimerFuture` wakes itself with `std::thread::spawn`, which single-threaded wasm32 cannot do.

### Other new features

- `bindings.typescript.forceAsync` in `uniffi.toml` gives chosen types and functions an `async`/`Promise` surface in TypeScript without making them async in Rust, so call sites can be migrated to `await` ahead of any real off-main-thread work. Set it to `true` for the whole crate, or to a list of names. See [the reference](https://jhugman.github.io/uniffi-bindgen-react-native/reference/uniffi-toml.html#forcing-an-async-surface) — in particular, it moves no work off the main thread ([#408](https://github.com/jhugman/uniffi-bindgen-react-native/pull/408)).
- `bindings.typescript.strictTypeChecking` drops the `// @ts-nocheck` header from generated files so `tsc` checks them ([#408](https://github.com/jhugman/uniffi-bindgen-react-native/pull/408)).
- Lifting and lowering now share a cursor instead of creating a `DataView` per value, which speeds up types made of many small reads and writes — large or deeply nested records, recursive enums — on every backend ([#413](https://github.com/jhugman/uniffi-bindgen-react-native/pull/413)).
- The supported `uniffi-rs` version is relaxed from `=0.31.0` to `=0.31`, so 0.31.x point releases no longer need a matching release here ([#431](https://github.com/jhugman/uniffi-bindgen-react-native/pull/431)).

## 🦊 What's Changed

### Memory leaks fixed

Four leaks affecting the N-API backend, present in 0.31.0-3. Upgrading is recommended for anyone making calls in a loop.

- The `RustBuffer` returned by an **async** call was never freed, leaking one buffer per async call returning a string, byte array, record or list. The `wasm` flavour was unaffected ([#420](https://github.com/jhugman/uniffi-bindgen-react-native/pull/420)).
- The N-API runtime allocated every `RustBuffer` **argument** twice and freed only one of the two, leaking a copy of each buffer passed into Rust — roughly 2.4 KB per frame. Lowered arguments are now library-owned rather than copied ([#432](https://github.com/jhugman/uniffi-bindgen-react-native/pull/432)). Thank you [@1egoman](https://github.com/1egoman)!
- Callback trampolines were rebuilt on every call that marshalled a callback argument, instead of once per callback type as intended. `rust_future_poll` marshals its continuation on every poll, so **every `await` of a uniffi async function** paid this — each construction leaking a pinned `napi_ref`, a `CallbackUserData`, and a fresh `ThreadsafeFunction` libuv handle. Trampolines are now cached per callback type ([#440](https://github.com/jhugman/uniffi-bindgen-react-native/pull/440)).
- Callback interfaces implemented in TypeScript leaked their whole serialized return value on each invocation, for any method returning a record, enum, `Vec`, `Option` or byte array. #432 taught the **argument** path to adopt library-owned buffers; the **callback-return** path still copied, orphaning the original. `String` returns were never affected — their converter produces `TextEncoder` memory rather than a library allocation ([#442](https://github.com/jhugman/uniffi-bindgen-react-native/pull/442)).

Alongside those, the runtime now refuses to free a buffer it cannot prove the library owns, rather than falling back to guessing the capacity from `byteLength`. Generated code always marks the buffers it frees, so this is hardening rather than a user-visible fix ([#441](https://github.com/jhugman/uniffi-bindgen-react-native/pull/441)).

### Fixes

- N-API: `is_js_thread` is now answered per callback rather than per process. Calling a library from a Node `worker_threads` worker — which is what Vitest and Jest do by default — hung on the first call, taking down `worker.terminate()` and `process.exit()` with it. Every async call was affected, not just calls with a callback of your own ([#433](https://github.com/jhugman/uniffi-bindgen-react-native/pull/433), fixing [#436](https://github.com/jhugman/uniffi-bindgen-react-native/issues/436)). Thank you [@stupside](https://github.com/stupside)!
- Escape `*/` in generated TypeScript docstrings. TypeScript doesn't allow nested block comments, so a `*/` in a doc comment closed the docstring early and the rest was parsed as code, failing the build ([#438](https://github.com/jhugman/uniffi-bindgen-react-native/pull/438)). Thank you [@liamiepops](https://github.com/liamiepops)!
- Export the `./package.json` subpath from the root package. The generated Android CMake resolves the package root with `require.resolve('uniffi-bindgen-react-native/package.json')`, which failed with `ERR_PACKAGE_PATH_NOT_EXPORTED` where Node enforces the `exports` map ([#407](https://github.com/jhugman/uniffi-bindgen-react-native/pull/407), fixing [#404](https://github.com/jhugman/uniffi-bindgen-react-native/issues/404)). Thank you [@DeyLak](https://github.com/DeyLak)!
- Android: replace the deprecated `TurboReactPackage` with `BaseReactPackage` in the generated Kotlin and Java packages ([#411](https://github.com/jhugman/uniffi-bindgen-react-native/pull/411), fixing [#410](https://github.com/jhugman/uniffi-bindgen-react-native/issues/410)). Thank you [@ANAMASGARD](https://github.com/ANAMASGARD)!
- Emit `opt-level = 3` rather than `opt-level = "3"` in the generated WASM template's release profile ([#401](https://github.com/jhugman/uniffi-bindgen-react-native/pull/401)). Thank you [@MrCreativ3001](https://github.com/MrCreativ3001)!

### CI

- The React Native compatibility matrix is now date-derived: React Native versions, `create-react-native-library` versions and runner images are tied together by date, covering the last 12 months of React Native. Per-PR checks run the latest versions only; the historical sweep runs nightly. This fixes the frequent build breakages caused by old React Native versions no longer building on `macos-latest` ([#419](https://github.com/jhugman/uniffi-bindgen-react-native/pull/419)).
- Node 24 in CI ([#439](https://github.com/jhugman/uniffi-bindgen-react-native/pull/439)).

## ⚠️ Breaking Changes

- The N-API `RustBuffer` fixes change the contract between the generated bindings and the runtime: regenerate your bindings and upgrade [`@ubjs/node`](https://www.npmjs.com/package/@ubjs/node) together. A new runtime with old bindings, or the reverse, will not behave correctly ([#420](https://github.com/jhugman/uniffi-bindgen-react-native/pull/420), [#432](https://github.com/jhugman/uniffi-bindgen-react-native/pull/432)).

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.31.0-3...0.31.0-5

---

# 0.31.0-3

## ✨ What's New ✨

### 🟢 Node.js!

`uniffi-bindgen-react-native` can now generate bindings that run directly on Node.js, joining React Native and the Web as a supported target. A new N-API runtime — published to npm as [`@ubjs/node`](https://www.npmjs.com/package/@ubjs/node) — loads your compiled Rust `cdylib` at runtime and calls into it, so a single prebuilt native addon works with any UniFFI library, with no per-library glue code to build. See the [Node.js reference](https://jhugman.github.io/uniffi-bindgen-react-native/reference/nodejs.html) for how to get started.

- A new N-API runtime added under `runtimes/` ([#369](https://github.com/jhugman/uniffi-bindgen-react-native/pull/369)), later refactored into layers ([#385](https://github.com/jhugman/uniffi-bindgen-react-native/pull/385)), with single-copy and string/byte optimizations across the FFI boundary ([#378](https://github.com/jhugman/uniffi-bindgen-react-native/pull/378), [#394](https://github.com/jhugman/uniffi-bindgen-react-native/pull/394)).
- A new `ubrn generate napi bindings` command (aliased `node`) generates the TypeScript bindings, with the location of the `cdylib` resolved at bindgen time via colocated, absolute, or platform-package modes ([#390](https://github.com/jhugman/uniffi-bindgen-react-native/pull/390), [#398](https://github.com/jhugman/uniffi-bindgen-react-native/pull/398)).
- The full fixture test suite now runs against the N-API flavour ([#377](https://github.com/jhugman/uniffi-bindgen-react-native/pull/377), [#382](https://github.com/jhugman/uniffi-bindgen-react-native/pull/382)), including on Linux via colima.
- `index.ts` generation added to the player pipeline ([#391](https://github.com/jhugman/uniffi-bindgen-react-native/pull/391)); callback function identifiers are now suffixed with the module name to avoid collisions across crates ([#393](https://github.com/jhugman/uniffi-bindgen-react-native/pull/393)). Thank you [@markharding](https://github.com/markharding)!
- Published to npm via CI ([#386](https://github.com/jhugman/uniffi-bindgen-react-native/pull/386)); the runtime package is now [`@ubjs/node`](https://www.npmjs.com/package/@ubjs/node) ([#396](https://github.com/jhugman/uniffi-bindgen-react-native/pull/396)) and the TypeScript runtime is now [`@ubjs/core`](https://www.npmjs.com/package/@ubjs/core) ([#399](https://github.com/jhugman/uniffi-bindgen-react-native/pull/399)).

- Byte arrays (`Vec<u8>`) can now be globally emitted as `Uint8Array` instead of `ArrayBuffer` by setting `bindings.typescript.strictByteArrays` in `uniffi.toml` ([#383](https://github.com/jhugman/uniffi-bindgen-react-native/pull/383) by [@coriolinus](https://github.com/coriolinus)).

## 🦊 What's Changed

- Migrate the `wrapper-ffi.ts` ([#359](https://github.com/jhugman/uniffi-bindgen-react-native/pull/359)) and `wrapper.ts` ([#363](https://github.com/jhugman/uniffi-bindgen-react-native/pull/363)) generation to the new `pipeline` API.
- Audit default-value positions and types, add a fixture, and fix related bugs ([#392](https://github.com/jhugman/uniffi-bindgen-react-native/pull/392)).
- Fix reserved C++ words being missed during template checking ([#389](https://github.com/jhugman/uniffi-bindgen-react-native/pull/389)). Thank you [@bjtrounson](https://github.com/bjtrounson)!
- Several Windows fixes — thank you [@pepperoni505](https://github.com/pepperoni505)!
  - Convert backslash paths to forward slashes ([#364](https://github.com/jhugman/uniffi-bindgen-react-native/pull/364)).
  - Strip the `\\?\` prefix from canonicalized paths ([#367](https://github.com/jhugman/uniffi-bindgen-react-native/pull/367)).
  - Don't prepend `lib` to DLLs ([#365](https://github.com/jhugman/uniffi-bindgen-react-native/pull/365)).
  - Fix `xtask bootstrap` so the tests run ([#373](https://github.com/jhugman/uniffi-bindgen-react-native/pull/373)).
- Have prettier only format JS and TS files ([#368](https://github.com/jhugman/uniffi-bindgen-react-native/pull/368)). Thank you [@pepperoni505](https://github.com/pepperoni505)!
- Remove redundant files from the published package bundle ([#381](https://github.com/jhugman/uniffi-bindgen-react-native/pull/381)). Thank you [@Simek](https://github.com/Simek)!
- Commit an auto-applied change to the lockfile ([#380](https://github.com/jhugman/uniffi-bindgen-react-native/pull/380)). Thank you [@AndrewFerr](https://github.com/AndrewFerr)!
- CI: cross-compile `x86_64-apple-darwin` on Apple Silicon ([#395](https://github.com/jhugman/uniffi-bindgen-react-native/pull/395)) and remove the zig-based cross compile ([#397](https://github.com/jhugman/uniffi-bindgen-react-native/pull/397)).

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.31.0-2...0.31.0-3

---

# 0.31.0-2

## ✨ What's New ✨

- Records and enums now support methods in TypeScript bindings, tracking uniffi 0.31's new value-type methods feature. Flat enums use direct method calls; tagged enums use a factory pattern ([#347](https://github.com/jhugman/uniffi-bindgen-react-native/pull/347)).

- Add support for opting out of generating TypeScript interfaces: `bindings.typescript.strictObjectTypes` in
  `uniffi.toml` ([#341](https://github.com/jhugman/uniffi-bindgen-react-native/pull/341) by
  [@SimonThormeyer](https://github.com/SimonThormeyer)).

- Optional fields in record types no longer need to be explicitly passed as `undefined` ([#358](https://github.com/jhugman/uniffi-bindgen-react-native/pull/358)). Thank you [@Psycarlo](https://github.com/Psycarlo)!

## 🦊 What's Changed

- Bump `uniffi-rs` to [0.31.0](https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md).
- Replace deprecated `SwiftBindingGenerator`/`KotlinBindingGenerator` with `bindings::generate` for iOS and Android native bindings.
- Pin `wasm-bindgen` to 0.2.100 to avoid a compile-time regression in `js-sys`.
- Enable the `wasm-unstable-single-threaded` feature of `uniffi_core` in the runtime crate, fixing a `Send` bound error when building WASM projects ([#356](https://github.com/jhugman/uniffi-bindgen-react-native/pull/356)). Thank you [@SimonThormeyer](https://github.com/SimonThormeyer)!
- Fix Windows path separators in generated `CMakeLists.txt`, which was preventing bindings generation on Windows ([#352](https://github.com/jhugman/uniffi-bindgen-react-native/pull/352)). Thank you [@DavJCosby](https://github.com/DavJCosby)!
- Rename the WASM template `Cargo.toml` to `Cargo.toml.txt` so Cargo doesn't pick it up when traversing dependencies ([#354](https://github.com/jhugman/uniffi-bindgen-react-native/pull/354)). Thank you [@marc2332](https://github.com/marc2332)!
- Stop emitting `[profile.release]` in generated WASM crates that live inside a workspace ([#350](https://github.com/jhugman/uniffi-bindgen-react-native/pull/350)).
- Canonicalize relative paths before comparing them, fixing path resolution issues on some setups ([#349](https://github.com/jhugman/uniffi-bindgen-react-native/pull/349)).
- Split the shared codegen extensions into separate modules for each target language (TypeScript, C++, Rust), preparing for the uniffi pipeline API ([#353](https://github.com/jhugman/uniffi-bindgen-react-native/pull/353)).
- Replace `xtask run` with a proc-macro test harness so fixture tests run with `cargo test` ([#355](https://github.com/jhugman/uniffi-bindgen-react-native/pull/355)).

## ⚠️ Breaking Changes

- uniffi 0.31 changes method checksums (the self type is no longer included), so bindings compiled against 0.30.x are not compatible with 0.31.x. Regenerate your bindings after upgrading.

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.30.0-1...0.31.0-2

---

# 0.30.0-1

## ✨ What's New ✨

- Add support for 16KB page size alignment on android (as required by Android 15 + Google Play by Nov 1, 2025) ([#294](https://github.com/jhugman/uniffi-bindgen-react-native/pull/294)). Thank you [@zzorba](https://github.com/zzorba)!
- Uniffi traits `Display`, `Debug`, `Eq`, `Hash`, and `Ord` now generate corresponding TypeScript methods for records and enums (they already worked for objects; `Ord`/`compareTo()` is also new for objects).
- Custom types can now be used as `Result` error types when wrapping enums or objects.
- Function/method argument defaults now work correctly for custom types (e.g. `Option<CustomType>` parameters can have `= None`).

## 🦊 What's Changed

- Build TS to JS before publish; ship compiled JS + types to avoid strict TS errors. Inspired by [#198](https://github.com/jhugman/uniffi-bindgen-react-native/pull/198) ([@hassankhan](https://github.com/hassankhan)); implemented in [#297](https://github.com/jhugman/uniffi-bindgen-react-native/pull/297) ([@EthanShoeDev](https://github.com/EthanShoeDev)).
- Bump `uniffi-rs` to [0.30.0](https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md).
- Changed RustBuffer `capacity` and `len` to `uint64_t`, fixing a crasher on 32-bit devices. ([#313](https://github.com/jhugman/uniffi-bindgen-react-native/pull/313)). Thank you [@sfourdrinier](https://github.com/sfourdrinier)!

## ⚠️ Breaking Changes

- `UniffiRustArcPtr` renamed to `UniffiGcObject` and `UnsafeMutableRawPointer` renamed to `UniffiHandle` in generated TypeScript bindings. Regenerate your bindings to pick up the new names.

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.29.3-1...0.30.0-1

---

# 0.29.3-1

## ✨ What's New ✨

- Support for dynamic libraries on Android ([#285](https://github.com/jhugman/uniffi-bindgen-react-native/pull/285)). Thank you [@exploIF](https://github.com/exploIF)!
- Add `RUSTFLAGS` command for web build ([#276](https://github.com/jhugman/uniffi-bindgen-react-native/pull/276)). Thank you [@zzorba](https://github.com/zzorba)!

## 🦊 What's Changed

- A fix for generating native Kotlin bindings ([#283](https://github.com/jhugman/uniffi-bindgen-react-native/pull/283))
- `serde-toml-merge` is version pinned ([#280](https://github.com/jhugman/uniffi-bindgen-react-native/pull/280))
- Export `FFIConverters` for errors ([#279](https://github.com/jhugman/uniffi-bindgen-react-native/pull/279))

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.29.3-0...0.29.3-1

# 0.29.3-0

## ✨ What's New ✨

### 🌏🕸️ WASM!

After 6 months of development, we are releasing the first version of `uniffi-bindgen-react-native` for use with WASM:

- Different fixtures running:
  - Fixtures `chronological` and `gc-callbacks-crasher` (#238)
  - Fixture `async-callbacks` (#237)
- Configuration file and `ubrn` command line:
  - Enable entrypoint and ts bindings directory to be customized for wasm (#259)
  - Add `ubrn build web --and-generate` command (#253)
  - Add CLI testing for `uniffi-bindgen-react-native` command. (#257)
  - Refactor of ubrn_cli into config and commands modules (#251)
- `uniffi-runtime-javascript` runtime, now on `crates.io`:
  - Add runtimeVersion to vary version of uniffi-runtime-javascript (#256)
  - Prepare uniffi-runtime-javascript crate for release (#248)

## 🦊 What's Changed

- Add default value for the --config option in all ubrn commands (#265)
- Change Windows path separators in CMakeLists.txt (#261)
- Bump `uniffi-rs` version to 0.29.3 (#267)
- Bump bob & RN versions (#242) and (#260)
- Run yarn pack as part of compatibility tests (#250)
- Add to "who is using" section of readme (#239)
- Fix wrong key name of `manifestPath` in docs (#240)

## ⚠️ Breaking Changes

- Bump Typescript version to 5.8, affecting `ArrayBuffer` (#271)

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.29.0-0...0.29.3-0

---

# 0.29.0-0

## 🦊 What's Changed

- Hot-reloading: ensure promises resolve, and callbacks are called after hot reload ([#232](https://github.com/jhugman/uniffi-bindgen-react-native/pull/232)).
  - Thank you [@matthieugayon](https://github.com/matthieugayon)!

## 🌏🕸️ WASM!

- Add support for Promises/Futures ([#221](https://github.com/jhugman/uniffi-bindgen-react-native/pull/221)).

## ⚠️ Breaking Changes

- Upgrade [`uniffi-rs` to version 0.29.0](https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md#v0290-backend-crates-v0290---2025-02-06).
    - There are several changes users of `uniffi-rs` (and `uniffi-bindgen-react-native`) should be aware; [a migration guide](https://mozilla.github.io/uniffi-rs/latest/Upgrading.html) is provided by the uniffi team.
    - Switching template engines from `askama` to `rinja`.

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-3...0.29.0-0

---

# 0.28.3-3
## ✨ What's New

* Add option to generate native swift bindings ([#214](https://github.com/jhugman/uniffi-bindgen-react-native/pull/214))
* Add option to generate native kotlin bindings ([#218](https://github.com/jhugman/uniffi-bindgen-react-native/pull/218))

## 🌏🕸️ WASM!

* Added support for synchronous callbacks ([#216](https://github.com/jhugman/uniffi-bindgen-react-native/pull/216)).

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-2...0.28.3-3

---

# 0.28.3-2
## ✨ What's New
* Add `--profile` build argument ([#192](https://github.com/jhugman/uniffi-bindgen-react-native/pull/192))
  * Thank you [@Johennes](https://github.com/Johennes)!

## 🦊 What's Changed

* Adjust template to allow for hot reload via metro of running apps ([#207](https://github.com/jhugman/uniffi-bindgen-react-native/pull/207)).
* Stabilise `require.resolve` by looking up `package.json` instead of entrypoint ([#200](https://github.com/jhugman/uniffi-bindgen-react-native/pull/200)).
  * Thank you [@hassankhan](https://github.com/hassankhan)!
* Split compat job by platform and version ([#211](https://github.com/jhugman/uniffi-bindgen-react-native/pull/211)).
  * This shows on the README.md if builder-bob or React Native has changed breaking the tutorial.
  * Thank you [@Johennes](https://github.com/Johennes)!
* Fixed GC'ing objects with callbacks intermittent crasher ([#208](https://github.com/jhugman/uniffi-bindgen-react-native/pull/208) and [#209](https://github.com/jhugman/uniffi-bindgen-react-native/pull/209))
* Reproducibly pick the same library file when using `--and-generate` ([#194](https://github.com/jhugman/uniffi-bindgen-react-native/pull/194))
  * Thank you [@Johennes](https://github.com/Johennes)!

## 🌏🕸️ WASM!
* Fixtures `coverall`, `custom-types-example`, `enum-types`, `trait-methods` ([#202](https://github.com/jhugman/uniffi-bindgen-react-native/pull/202)).
* Switched from passing `ArrayBuffer`s to using `Uint8Array`, to accommodate WASM better. ([#187](https://github.com/jhugman/uniffi-bindgen-react-native/pull/187))
Callbacks now have UniffiResult to communicate between typescript and C++ ([#205](https://github.com/jhugman/uniffi-bindgen-react-native/pull/205)).
* Fixtures `coverall2` and `rondpoint` ([#191](https://github.com/jhugman/uniffi-bindgen-react-native/pull/191)).
* Fixture `arithmetic` ([#188](https://github.com/jhugman/uniffi-bindgen-react-native/pull/188)).

## 📰 Documentation
* Remove duplicate parentheses ([#203](https://github.com/jhugman/uniffi-bindgen-react-native/pull/203)).
* Minor typo fixes in GC docs ([#204](https://github.com/jhugman/uniffi-bindgen-react-native/pull/204)).
* Remove reference to name field in the ubrn.config.yaml docs ([#189](https://github.com/jhugman/uniffi-bindgen-react-native/pull/189)).

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-1...0.28.3-2

# 0.28.3-1

This is the first supported release of the `uniffi-bindgen-react-native`. Please hack responsibly. Share and enjoy.

## 🦊 What's Changed
* Handle type parameter change in crnl 0.45.1 ([#182](https://github.com/jhugman/uniffi-bindgen-react-native/pull/182))
* Make first run more informative while compiling ([#185](https://github.com/jhugman/uniffi-bindgen-react-native/pull/185))
* Initial refactor in preparing for WASM ([#174](https://github.com/jhugman/uniffi-bindgen-react-native/pull/174))
* Add callbacks-example fixture from uniffi-rs ([#172](https://github.com/jhugman/uniffi-bindgen-react-native/pull/172))
* Fix CLI working without an extension ([#183](https://github.com/jhugman/uniffi-bindgen-react-native/pull/183))
* Use version released to Cocoapods and npm ([#184](https://github.com/jhugman/uniffi-bindgen-react-native/pull/184))

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-0...0.28.3-1


[//]: # (## ✨ What's New)
[//]: # (## 🦊 What's Changed)
[//]: # (## ⚠️ Breaking Changes)
[//]: # (**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/{{previous}}...{{current}})
