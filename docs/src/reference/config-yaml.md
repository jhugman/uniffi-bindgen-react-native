# Configuration for `uniffi-bindgen-react-native`

The configuration yaml file is a collection of configuration options used in one or more [commands](commandline.md).

The file is designed to be easy to start. A **minimal** configuation would be:

```yaml
rust:
  directory: ./rust
  manifest-file: Cargo.toml
```

Getting started from here would require a command to start the Rust:

```sh
cargo init --lib ./rust
cd ./rust
cargo add uniffi
```

# YAML entries

## `rust`

```yaml
rust:
	repo: https://github.com/example/my-rust-sdk
	branch: main
	manifest-path: crates/my-api/Cargo.toml
```
In this case, the `ubrn checkout` command will clone the given repo with the branch/ref into the `rust_modules` directory of the project. Note that instead of `branch` you can also use `rev` or `ref`.

If run a second time, no overwriting will occur.

The `manifest-path` is the path relative to the root of the Rust workspace directory. In this case, the manifest is expected to be, relative to your React Native library project: `./rust_modules/my-rust-sdk/crates/my-api/Cargo.tml`.

```yaml
rust:
	directory: ./rust
	manifest-path: crates/my-api/Cargo.toml
```
In this case, the `./rust` directory tells `ubrn` where the Rust workspace is, relative to your React Native library project. The `manifest-path` is the relative path from the workspace file to the crate which will be used to build bindings.

## `bindings`

This section governs the generation of the bindings— the nitty-gritty of the Rust API translated into Typescript. This is mostly the location on disk of where these files will end up, but also has a second configuration file.

```yaml
bindings:
	cpp: cpp/bindings
	ts: ts/bindings
	uniffiToml: ./uniffi.toml
```
The [`uniffi.toml` file](uniffi-toml.md) configures custom types, to further customize the conversion into Typescript data-types.

If missing, the defaults will be used:
```
bindings:
	cpp: cpp/generated
	ts: ts/generated
```
## `android`

This is to configure the build steps for the Rust, the bindings, and the turbo-module code for Android.

This section can be omitted entirely, as sensible defaults are provided. If you do want to edit the defaults, these are the members of the `android` section with their defaults:

```yaml
android:
	directory: ./android
	cargoExtras: []
	targets:
	- arm64-v8a
	- armeabi-v7a
	- x86
	- x86_64
	apiLevel: 21
	jniLibs: src/main/jniLibs
	packageName: <DERIVED FROM package.json>
	codegenOutputDir: <DERIVED FROM package.json>
```

The `directory` is the location of the Android project, relative to the root of the React Native library project.

`targets` is a list of targets to build for. The Rust source code is built once per target.

`cargoExtras` is a list of extra arguments passed directly to the `cargo build` command.

`apiLevel` is the minimum API level to target: this is passed to the `cargo ndk` command as a `--platform` argument.

```admonish tip
Reducing the number of targets to build for will speed up the edit-compile-run cycle.
```

`packageName` is the name of the Android package that Codegen used to generate the TurboModule. `codegenOutputDir` is the path under which Codegen stores its generated files. Both are derived from the `package.json` file, and can almost always be left.

To customize the `packageName`, you should edit or add the entry at the path `codegenConfig`/`android`/`javaPackageName` in `package.json`.

To customize the `codegenOutputDir`, you should edit or add the entry at the path `codegenConfig`/`outputDir`/`android` in `package.json`.

```admonish warning
Note that for Android the `outputDir` value in `package.json` needs to have a matching entry under `dependency`/`platforms`/`android`/`cmakeListsPath` in `react-native.config.js`. For example, if you set the Android output directory in `package.json` to `android/tmp`, the `cmakeListsPath` value in `react-native.config.js` needs to be set to `tmp/jni/CMakeLists.txt`.
```

## `ios`

This is to configure the build steps for the Rust, the bindings, and the turbo-module code for iOS.

This section can be omitted entirely, as sensible defaults are provided. If you do want to edit the defaults, these are the members of the `ios` section with their defaults:

```yaml
ios:
	directory: ios
	cargoExtras:: []
	targets:
	- aarch64-apple-ios
	- aarch64-apple-ios-sim
	xcodebuildExtras: []
	frameworkName: build/MyFramework
	codegenOutputDir: <DERIVED FROM package.json>
```


The `directory` is the location of the iOS project, relative to the root of the React Native library project.

`targets` is a list of targets to build for. The Rust source code is built once per target.

`cargoExtras` is a list of extra arguments passed directly to the `cargo build` command.

`xcodebuildExtras` is a list of extra arguments passed directly to the `xcodebuild` command.

`codegenOutputDir` is the path under which Codegen stores its generated files. This is derived from the `package.json` file, and can almost always be left.

To customize the `codegenOutputDir`, you should edit or add the entry at the path `codegenConfig`/`outputDir`/`ios` in `package.json`.

```admonish warning
Note that for Android the `outputDir` value in `package.json` needs to have a matching entry under `dependency`/`platforms`/`android`/`cmakeListsPath` in `react-native.config.js`. For example, if you set the Android output directory in `package.json` to `android/tmp`, the `cmakeListsPath` value in `react-native.config.js` needs to be set to `tmp/jni/CMakeLists.txt`.
```

## `turboModule`

This section configures the location of the Typescript and C++ files generated by the `generate turbo-module` command.

If absent, the defaults will be used:

```yaml
turboModule:
    ts: src
    cpp: cpp
```

The Typescript files are the `index.ts` file, and the `Codegen` installer file.

```admonish info
By default, the `index.ts` file is intended to be the entry point for your library.

In this case, changing the location of the `ts` directory will require changing the `main` or `react-native` entry in the `package.json` file.
```

## `noOverwrite`

This list of [glob patterns](https://en.wikipedia.org/wiki/Glob_(programming)) of file that should not be generated or overwritten by the `--and-generate` flag, and the `generate turbo-module` command.

This is useful if you have customized one or more of the generated files, and do not want lose those changes.

For example, if you want to add C++ files to the library, you may want to change the build files.

```yaml
noOverwrite:
    - "*.podspec"
    - CMakeLists.txt
```

You can generate the build files once then not overwrite them. Once you excluded the files, they can be safely edited.
