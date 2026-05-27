# Cutting a Release

A release publishes four artifacts to three registries. All of them are
triggered automatically when a GitHub Release is _published_, so the bulk of
cutting a release is: get the version numbers right, land the bump, then draft
the release.

## What gets published

| Artifact | Registry | Workflow | Source | Version comes from |
| --- | --- | --- | --- | --- |
| `uniffi-bindgen-react-native` | npm | [`npm.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/npm.yml) | repo root | `package.json` |
| `@ubjs/node` + `@ubjs/node-<platform>` | npm | [`napi-publish.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/napi-publish.yml) | `runtimes/napi` | `runtimes/napi/package.json` |
| `uniffi-runtime-javascript` | crates.io | [`crates-io.yaml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/crates-io.yaml) | `crates/uniffi-runtime-javascript` | `crates/uniffi-runtime-javascript/Cargo.toml` |
| `uniffi-bindgen-react-native` (Pod) | CocoaPods | [`cocoapods.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/cocoapods.yml) | `uniffi-bindgen-react-native.podspec` | `package.json` (the podspec reads `package['version']`) |

`@ubjs/node` is the N-API runtime. Its workflow first builds a native binary
for every supported target (macOS x64/arm64, Linux gnu/musl on x64/arm64,
Windows x64/arm64), publishes each as a platform package
(`@ubjs/node-darwin-arm64`, `@ubjs/node-linux-x64-gnu`, …), then publishes the
`@ubjs/node` root package whose `optionalDependencies` point at them. If the
build matrix fails for any target, the publish job does not run.

## Steps

1. Increment the version number, keeping all four files in sync:
   - [`package.json`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/package.json#L3) (also drives the CocoaPod)
   - [`crates/ubrn_cli/Cargo.toml`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_cli/Cargo.toml#L3)
   - [`crates/uniffi-runtime-javascript/Cargo.toml`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/uniffi-runtime-javascript/Cargo.toml#L3)
   - [`runtimes/napi/package.json`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/napi/package.json#L3) (the `@ubjs/node` runtime)
1. Update the CHANGELOG. If the CHANGELOG is up-to-date, then this should be minimal.
   - Add a new version title at the top
   - Update the Full Changelog link to go from new release to main
   - Move the bottom of the "upcoming release" section to the top
   - Update the Full Changelog link to go from previous release to new release
1. Push as a PR as usual, with subject: `Release ${VERSION_NUMBER}`.
1. (Optional but recommended) Run a dry-run of the publish workflows — see
   [Testing a release before tagging](#testing-a-release-before-tagging).
1. Once the PR has landed, [draft a new release](https://github.com/jhugman/uniffi-bindgen-react-native/releases/new).
1. Create a new tag (in the choose-a-tag dialog) with the version number (without a `v`).
1. Use the version number again, but with a `v` prepended, for the release title: `v${VERSION_NUMBER}`.
1. Publish the release.
1. Wait for the four publish workflows to go green:
   - [CocoaPods](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/cocoapods.yml)
   - [npm — `uniffi-bindgen-react-native`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/npm.yml)
   - [npm — `@ubjs/node`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/napi-publish.yml)
   - [crates.io](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/crates-io.yaml)
1. [Verify the release landed](#after-publishing).
1. Tell your friends, make a song and dance, you've done a new release.

## Testing a release before tagging

Three of the four publish workflows can be run manually from the Actions tab
(`workflow_dispatch`) with a **dry-run** input that defaults to `true`. Use this
to validate packaging — `cargo publish --dry-run`, `npm publish --dry-run` —
without pushing anything to a registry:

- [`npm.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/npm.yml) — `dry-run` input
- [`crates-io.yaml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/crates-io.yaml) — `dry_run` input
- [`napi-publish.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/napi-publish.yml) — `dry-run` input

```admonish warning
[`cocoapods.yml`](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/cocoapods.yml)
has **no** dry-run input. Triggering it manually runs a real `pod trunk push`.
To only validate the podspec, run `pod spec lint uniffi-bindgen-react-native.podspec`
locally instead.
```

A real release fires every workflow on the `release: published` event; the
dry-run path is reachable only through manual `workflow_dispatch`.

## After publishing

Confirm each artifact actually went out:

- npm: `npm view uniffi-bindgen-react-native version` and `npm view @ubjs/node version`
- crates.io: <https://crates.io/crates/uniffi-runtime-javascript/versions>
- CocoaPods: `pod trunk info uniffi-bindgen-react-native`

If a workflow fails part-way, re-run just that workflow from the Actions tab
(`workflow_dispatch`, dry-run `false`) once the underlying problem is fixed —
you do not need to cut a new tag. npm and crates.io reject re-publishing a
version that already exists, so a re-run after a partial `@ubjs/node` publish
will skip the platform packages that already landed and publish the rest.

## Version numbers

`uniffi-rs` uses a `semver` versioning scheme (e.g. `0.31.0`).

`uniffi-bindgen-react-native` tracks the `uniffi-rs` version and appends a `-N` variant number. The variant number increases monotonically across releases and is **not** reset when the `uniffi-rs` version changes.

For example, if the last release was `0.30.0-1` and `uniffi-rs` is bumped to `0.31.0`, the next release is `0.31.0-2` (not `0.31.0-0`).

### Compatibility with other packages

Other versioning we should take care to note:

- React Native
- `create-react-native-library`

Compatibility matrices are built during CI:

- [Android](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-android.yml?query=branch%3Amain)
- [iOS](https://github.com/jhugman/uniffi-bindgen-react-native/actions/workflows/compat-ios.yml?query=branch%3Amain)
