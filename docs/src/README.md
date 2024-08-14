[`uniffi-rs`](https://github.com/mozilla/uniffi-rs/blob/main/README.md) is a suite of projects to allow Rust to be used from other languages. It was started at Mozilla to facilitate building cross-platform components in Rust which could be run on Android and iOS.

It has since grown to support for other languages not in use at Mozilla.

![React Native Logo](images/react-native-logo.svg)
+
![Rust Logo](images/rust-logo.svg)

[`uniffi-bindgen-react-native`](https://github.com/jhugman/uniffi-bindgen-react-native) is the project that houses the bindings generators for react-native.

It contains tooling to generate bindings from Hermes via JSI, and to generate the code to create turbo-modules.

```admonish warning
This project is still in early development, and should not yet be used in production.
```
