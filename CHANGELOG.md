# Upcoming releases

[//]: # (## 🦊 What's Changed)
[//]: # (## ⚠️ Breaking Changes)
[//]: # (**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/{{previous}}...{{current}})

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-4...releases/v0.28.x

---

# 0.28.3-5

## 🦊 What's Changed

- Republishing v0.28.3.4 because Cocoapods timeout.

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-4...0.28.3-5

---

# 0.28.3-4
## 🦊 What's Changed

- Hot-reloading: ensure promises resolve, and callbacks are called after hot reload ([#232](https://github.com/jhugman/uniffi-bindgen-react-native/pull/232)).
  - Thank you [@matthieugayon](https://github.com/matthieugayon)!

**Full Changelog**: https://github.com/jhugman/uniffi-bindgen-react-native/compare/0.28.3-3...0.28.3-4

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
