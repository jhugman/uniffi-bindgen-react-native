# Publishing your library project

```admonish warning title="Help wanted"
I haven't had any experience of publishing libraries for React Native.

I would love some [help with this document](../contributing/documentation.md).
```

## Binary builds

You likely don't want to track pre-built binaries in your git repository but you may want to include them in published packages. If so, you will have to work around `npm`'s behaviour of factoring in `.gitignore` when picking files to include in packages.

One way to do this is by using the `files` array in `package.json`. The steps to achieve this will depend on the particular contents of your repository. For illustration purposes, let's assume you're ignoring binaries in `.gitignore` with

```
build/
*.a
```

To include the libraries in `/build/$library.xcframework` and `/android/src/main/jniLibs/$target/$library.a` in your npm package, you can add the following to `package.json`:

```diff
"files": [
+ "android",
+ "build",
```

Another option is to create an `.npmignore` file. This will require you to duplicate most of the contents of `.gitignore` though and might create issues if you forget to duplicate entries as you add them later.

In either case, it's good practice to run `npm pack --dry-run` and verify the package contents before publishing.

## Source packages

If asking your users to compile Rust source is acceptable, then adding a `postinstall` script to `package.json` may be enough.

If you've kept the scripts from the [Getting Started guide](./getting-started.md#step-2-add-uniffi-bindgen-react-native-to-the-project), then adding:

```diff
scripts: {
  "scripts": {
    "ubrn:ios":      "ubrn build ios     --config ubrn.config.yaml --and-generate && (cd example/ios && pod install)",
    "ubrn:android":  "ubrn build android --config ubrn.config.yaml --and-generate",
    "ubrn:checkout": "ubrn checkout      --config ubrn.config.yaml",
+   "postinstall":   "yarn ubrn:checkout && yarn ubrn:android --release && yarn ubrn:ios --release",
```

## Add `uniffi-bindgen-react-native` to your README.md

If you publish your source code anywhere, it would be lovely if you could add something to your README.md. For example:

```diff
Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
+ and [uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native)
```

## Add your project to the `uniffi-bindgen-react-native` README.md

Once your project is published and would like some cross-promotion, perhaps you'd like to raise a PR to add it to the [`uniffi-bindgen-react-native` README](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/README.md#who-is-using-uniffi-bindgen-react-native).
