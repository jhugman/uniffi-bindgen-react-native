# Cutting a Release

1. The version number should be incremented in:
   - the [`package.json`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/package.json#L3)
   - the [`crates/ubrn_cli/Cargo.toml`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_cli/Cargo.toml#L3).
   - the [`crates/uniffi-runtime-javascript`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/uniffi-runtime-javascript/Cargo.toml#L3)
1. Push as a PR as usual, with subject: `Release ${VERSION_NUMBER}`.
1. Once this has landed, [draft a new release](https://github.com/jhugman/uniffi-bindgen-react-native/releases/new).
1. Create a new tag (in the choose a new tag dialog) with the version number (without a `v`).
1. Use the version number again, but with a `v` prepended for the release title, `v${VERSION_NUMBER}`.
1. Publish the release.
1. Wait until the various package managers have been told:
   - [Cocoapods](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/cocoapods.yml)
   - [NPM](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/npm.yml)
   - [`crates.io`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/crate-io.yml)
1. Tell your friends, make a song and dance, you've done a new release.

## Version numbers

Uniffi has a `semver` versioning scheme. At time of writing, the current version of `uniffi-rs` is `0.28.3`

`uniffi-bindgen-react-native` uses this version number with prepended with a `-` and a variant number, starting at `0`.

Thus, at first release, the version of `uniffi-bindgen-react-native` was `0.28.3-0`.

### Compatibility with other packages

Other versioning we should take care to note:

- React Native
- `create-react-native-library`

A version matrix is built during CI: [![version compatibility](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat.yml/badge.svg?query=branch%3Amain) compatibility matrix](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat.yml?query=branch%3Amain).
