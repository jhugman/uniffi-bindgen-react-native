# Upcoming releases

[//]: # (## ✨ What's New ✨)
[//]: # (## 🦊 What's Changed)
[//]: # (## ⚠️ Breaking Changes)
[//]: # (**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/{{previous}}...{{current}})

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.31.0-2...main

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
