# The JSI player (`jsi2`)

The `jsi2` flavour loads your Rust library through one native module the app
installs once, `@ubjs/react-native`. Your library ships no native code of its
own: TypeScript, a shared library per Android ABI, and a dynamic framework for
iOS. App developers need Xcode or the NDK, which every React Native app needs
already, and never a Rust toolchain.

## For app developers

```sh
npm install @ubjs/react-native
```

It must be a **direct** dependency of the app: React Native autolinking reads
only the app's own `package.json`. Then `pod install` or a Gradle sync. Every
library generated with `ubrn jsi2` declares it as a peer dependency and fails
loudly when it is missing: `pod install` cannot resolve the `UbjsReactNative`
pod, Gradle cannot find the `:ubjs_react-native` project, and if both are
bypassed the library throws at import.

## For library authors

The same toolchain as the `jsi` flavour: rustup, the iOS and Android targets,
and `cargo-ndk` (see [Before you start](../guides/rn/pre-installation.md)).
Your crate must build a `cdylib`:

```toml
[lib]
crate-type = ["lib", "cdylib"]
```

`ubrn` refuses to build a crate that does not.

### Configuration

A `jsi2:` section in `ubrn.config.yaml`; every key is optional.

```yaml
rust:
  directory: rust
  manifestPath: Cargo.toml
jsi2:
  # Where the TypeScript bindings go. Defaults to bindings.ts (src/generated).
  ts: src/generated
  # The Android ABIs the library ships. 64-bit only by default.
  androidTargets: [arm64-v8a, x86_64]
  # The iOS deployment target of the dylib and its framework.
  minIosVersion: "15.1"
  # Reverse-DNS prefix of the framework's bundle identifier. Defaults to the
  # Android package name.
  bundleIdPrefix: com.example
```

`android:` and `ios:` still supply the platform directories, the Android API
level, cargo extras and the iOS target list.

### Building and generating

```sh
ubrn build jsi2 android --and-generate            # lib<name>.so per ABI into android/src/main/jniLibs
ubrn build jsi2 ios --and-generate                # ios/<name>.xcframework of <name>.framework bundles
ubrn generate jsi2 all target/debug/lib<name>.dylib   # the TypeScript and packaging only
```

`<name>` is your crate's cdylib name, what cargo puts in `lib<name>.dylib`.
Every module of the library opens the library by that name; the player maps
it to `lib<name>.so` on Android and the embedded `<name>.framework` on iOS.

`generate all` writes:

| file | |
| --- | --- |
| `src/index.tsx` | imports `@ubjs/react-native` first, guards its absence, re-exports and initialises each module |
| `src/generated/*.ts` | the bindings |
| `<Name>.podspec` | vendors `ios/<name>.xcframework`, depends on `UbjsReactNative` |
| `android/build.gradle`, `android/src/main/AndroidManifest.xml` | package `jniLibs`; depend on `:ubjs_react-native` |
| `android/src/main/java/<pkg>/<Name>Package.java` | an empty `ReactPackage`: autolinking includes a library only if it has one |
| `package.json` | gains `peerDependencies["@ubjs/react-native"]` and `dependencies["@ubjs/core"]` when missing |

## Release check on a device

The simulator never signs anything, so before a release run the library's app
on a physical iPhone once:

1. Build with `ubrn build jsi2 ios --and-generate` (device and simulator slices).
2. In the app's Xcode project, select a development team and a physical device, and run.
3. Confirm the framework was embedded and signed with the app's identity:

   ```sh
   APP=$(ls -d ~/Library/Developer/Xcode/DerivedData/*/Build/Products/Debug-iphoneos/*.app | head -1)
   codesign -dv --verbose=2 "$APP/Frameworks/<name>.framework"
   ```

   Expected: `Identifier=<bundle id>`, `Authority=Apple Development: ...`, and
   no `code object is not signed at all` error.
4. Call one function of the library and see its result on screen or in the console.
