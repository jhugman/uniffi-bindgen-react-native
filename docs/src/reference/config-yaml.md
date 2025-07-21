# Configuration for `uniffi-bindgen-react-native`

The configuration yaml file is a collection of configuration options used in one or more [commands](commandline.md).

The file is designed to be easy to start. A **minimal** configuation would be:

```yaml
rust:
  directory: ./rust
  manifestPath: Cargo.toml
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
    manifestPath: crates/my-api/Cargo.toml
```
In this case, the `ubrn checkout` command will clone the given repo with the branch/ref into the `rust_modules` directory of the project. Note that instead of `branch` you can also use `rev` or `ref`.

If run a second time, no overwriting will occur.

The `manifestPath` is the path relative to the root of the Rust workspace directory. In this case, the manifest is expected to be, relative to your React Native library project: `./rust_modules/my-rust-sdk/crates/my-api/Cargo.tml`.

```yaml
rust:
    directory: ./rust
    manifestPath: crates/my-api/Cargo.toml
```
In this case, the `./rust` directory tells `ubrn` where the Rust workspace is, relative to your React Native library project. The `manifestPath` is the relative path from the workspace file to the crate which will be used to build bindings.

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
    useSharedLibrary: true
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

`useSharedLibrary` is a boolean that controls if the Rust code is linked as a shared library or a static library. The default is `false`, which means that the Rust code is linked as a static library. If you want to linked it as a dynamic library, set this to `true`.

````admonish warning
Note that when building as a shared library, you should ensure that Rust is configured to build dynamic library.

```toml
[lib]
crate-type = ["cdylib"]
```

Also, please keep in mind that with `useSharedLibrary: true`, you should not `strip` your library while building it. As this will break generating turbo module and native bindings. This should not affect app performance as Android will optimize it during app build.
```toml
[profile.your_profile]
strip = "none"
```
````

## `ios`

This is to configure the build steps for the Rust, the bindings, and the turbo-module code for iOS.

This section can be omitted entirely, as sensible defaults are provided. If you do want to edit the defaults, these are the members of the `ios` section with their defaults:

```yaml
ios:
    directory: ios
    cargoExtras: []
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

## `web`

This is to configure the build steps for the Rust, the bindings, and the turbo-module code for iOS.

This section can be omitted entirely, as sensible defaults are provided. If you do want to edit the defaults, these are the members of the `ios` section with their defaults:

```yaml
web:
    manifestPath: rust_modules/wasm/Cargo.toml
    manifestPatchFile: null
    wasmCrateName: <DERIVED FROM package.json>
    features: []
    defaultFeatures: true
    workspace: false
    runtimeVersion: <DERIVED FROM UBRN>
    cargoExtras: []
    target: web
    wasmBindgenExtras: []
    entrypoint: <DERIVED FROM package.json> or "src/index.web.ts"
    tsBindings: <SAME AS bindings/ts>

```

The `manifestPath` is the path to the generated wasm-crate. The location of paths for the `generate wasm wasm-crate` and for the rust files for `generate wasm bindings` are derived from this path.

The `manifestPatchFile` is a path to a TOML file that will be used to patch merge on top of the generated wasm-crate `Cargo.toml`. This is extremely useful when customizing the manifest. e.g. when [overriding dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html) in the target crate.

The `wasmCrateName` is the name of the wasm-crate, derived from the `package.json` `name` property.

The `features` array is used to build the target crate, _and_ then added to the wasm crate's `Cargo.toml`.

The `defaultFeatures` flag pairs with the `features` array: it is used to build the target crate (toggling the `--no-default-features` command line option) and then added to the wasm crate's `Cargo.toml`, as `default-features`.

The boolean `workspace` controls if the wasm crate is part of [an existing Rust workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html). The default value assumes that the target crate doesn't know anything about the wasm-crate, so the wasm-crate is in its own workspace. If the target crate is in a workspace, and that can be changed, then this setting can be changed to `true`. Tip: [`members` can contain globs](https://doc.rust-lang.org/cargo/reference/workspaces.html#:~:text=the%20members%20list%20also%20supports%20globs) to point to crates that don't yet exist.

`runtimeVersion` is the version of [`uniffi-runtime-javascript` crate](https://crates.io/crates/uniffi-runtime-javascript). By default this is the exact current version of uniffi-bindgen-react-native, so currently `=0.29.0-1`.

`cargoExtras` is a list of extra arguments passed directly to the `cargo build` command when building for `wasm32-unknown-unknown`.

`target` is the passed to `wasm-bindgen` or `wasm-pack`. Default is `web`. This is likely only useful if you're customizing the way the WASM bundle is loaded.

`wasmBindgenExtras` is a list of extra arguments passed directly to `wasm-bindgen`.

`entrypoint` is the filepath of the file which loads and exports the bindings. By default this is taken from the `browser` property of `package.json`, or `src/index.web.ts` if that is missing.

`tsBindings` is the directory where the typescript bindings are generated. This overrides the [`bindings`/`ts`](#bindings) directory.

```admonish warning
Uniffi is unable to process WASM files directly, so has to use a `lib.a` file built for the build environment.

Any `uniffi::export` or `uniffi` derive macros should not be toggled on and off based on the target architecture. If you want wasm specific uniffi bindings, you should use a `feature` instead, and add it to the `features` list in this file.
```

## `turboModule`

This section configures the location of the Typescript and C++ files generated by the `generate jsi turbo-module` command.

If absent, the defaults will be used:

```yaml
turboModule:
    cpp: cpp
    ts: <DERIVED FROM package.json>
    spec: <DERIVED FROM package.json>
    entrypoint: <DERIVED FROM package.json>
```

The default `entrypoint` is derived from the `react-native` entry in the `package.json`, and if missing, `src/index.tsx`.

The `spec` is the name of the Codegen spec, e.g. `NativeModule`. By default it is derived from the `codegenConfig`/`name` property in `package.json`.

The `ts` directory is used for the Codegen spec `NativeModule.ts`, and is the default is taken from the `codegenConfig`/`jsSrcDir` property in `package.json`.

The Typescript files are the `index.tsx` file, and the `Codegen` installer file.

```admonish info
By default, the `index.tsx` file is intended to be the entry point for your library.

If this is not the case—e.g. you want to do use the Rust as part of a larger library, then change the `entrypoint` to something other than the `package.json` value.
```

## `noOverwrite`

This list of [glob patterns](https://en.wikipedia.org/wiki/Glob_(programming)) of file that should not be generated or overwritten by the `--and-generate` flag, and the `generate jsi turbo-module` and `generate wasm wasm-crate` commands.

This is useful if you have customized one or more of the generated files, and do not want lose those changes.

For example, if you want to add C++ files to the library, you may want to change the build files.

```yaml
noOverwrite:
    - "*.podspec"
    - CMakeLists.txt
```

You can generate the build files once then not overwrite them. Once you excluded the files, they can be safely edited.
