# Cutting a Release

## Version numbers

Uniffi has a `semver` versioning scheme. At time of writing, the current version of `uniffi-rs` is `0.28.3`

`uniffi-bindgen-react-native` uses this version number with prepended with a `-` and a variant number, starting at `0`.

Thus, at first release, the version of `uniffi-bindgen-react-native` will be `0.28.3-0`.

### Compatibility with other packages

Other versioning we should take care to note:

- React Native
- `create-react-native-library`
