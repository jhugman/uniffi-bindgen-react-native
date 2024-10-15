# Publishing your library project

```admonish warning title="Help wanted"
I haven't had any experience of publishing libraries for React Native.

I would love some [help with this document](../contributing/documentation.md).
```

## Binary builds

In order to distribute pre-built packages you will need to add to `.gitignore` the built `.a`:

```diff
# From uniffi-bindgen-react-native
rust_modules/
+*.a
```

but add them back in to the `files` section of `package.json`[^issue121].

[^issue121]: [This advice](https://github.com/jhugman/uniffi-bindgen-react-native/issues/121) is from @Johennes

## Source packages

If asking your users to compile Rust source is acceptable, then adding a `postinstall` script to `package.json` may be enough.

If you've kept the scripts from the [Getting Started](./guide.md#step-2-add-uniffi-bindgen-react-native-to-the-project), then adding:

```diff
scripts: {
  "scripts": {
    "ubrn:ios":      "ubrn build ios     --config ubrn.config.yaml --and-generate && (cd example/ios && pod install)",
    "ubrn:android":  "ubrn build android --config ubrn.config.yaml --and-generate",
    "ubrn:checkout": "ubrn checkout      --config ubrn.config.yaml",
+    "postinstall": "yarn ubrn:checkout && yarn ubrn:android && yarn ubrn:ios",
```

## Add `uniffi-bindgen-react-native` to your README.md

If you publish your source code anywhere, it would be lovely if you could add something to your README.md. For example:

```diff
Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
+ and [uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native)
```

## Add your project to the `uniffi-bindgen-react-native` README.md

Once your project is published and would like some cross-promotion, perhaps you'd like to raise a PR to add it to the [`uniffi-bindgen-react-native` README](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/README.md#who-is-using-uniffi-bindgen-react-native).
