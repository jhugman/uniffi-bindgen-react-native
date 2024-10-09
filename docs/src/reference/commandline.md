`uniffi-bindgen-react-native` the command line utility that ties together much of the building of Rust, and the generating the bindings and turbo-modules. It is also available called `ubrn`.

Most commands take a `--config FILE` option. This is a YAML file which collects commonly used options together, and is [documented here](config-yaml.md).

Both spellings of the command `ubrn` and `uniffi-bindgen-react-native` are NodeJS scripts.

This makes `ubrn` available to other scripts in `package.json`.

If you find yourself running commands from the command line, you can alias the command

```bash
alias ubrn=$(yarn uniffi-bindgen-react-native --path)
```

allows you to run the command from the shell as `ubrn`, which is simpler to type. From hereon, commands will be given as `ubrn` commands.


# The `ubrn` command

Running `ubrn --help` gives the following output:

```sh
Usage: uniffi-bindgen-react-native <COMMAND>

Commands:
  checkout  Checkout a given Github repo into `rust_modules`
  build     Build (and optionally generate code) for Android or iOS
  generate  Generate bindings or the turbo-module glue code from the Rust
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## `checkout`
Checkout a given Git repo into `rust_modules`.

```sh
Usage: uniffi-bindgen-react-native checkout [OPTIONS] <REPO>

Arguments:
  <REPO>  The repository where to get the crate

Options:
      --config <CONFIG>
      --branch <BRANCH>  The branch or tag which to checkout [default: main]
  -h, --help             Print help
```
The checkout command can be operated in two ways, either:
1. with a `REPO` argument and optional `--branch` argument. OR
2. with a [config file][config] which may specify a repo and branch, or just a `directory`.

If the config file is set to a repo, then the repo is cloned in to `./rust_modules/${NAME}`.

# `build`

[config]: config-yaml.md

This takes care of the work of compiling the Rust, ready for generating bindings. Each variant takes a:

- `--config` [config file][config].
- `--and-generate` this runs the `generate all` command immediately after building.
- `--targets` a comma separated list of targets, specific to each platform. This overrides the values in the config file.
- `--release` builds a release version of the library.

## `build android`

Build the crate for use on an Android device or emulator, using `cargo ndk`, which in turn uses Android Native Development Kit.

```
Usage: uniffi-bindgen-react-native build android [OPTIONS] --config <CONFIG>

Options:
      --config <CONFIG>
          The configuration file for this build

  -t, --targets <TARGETS>...
          Comma separated list of targets, that override the values in the `config.yaml` file.

          Android: aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android i686-linux-android,

          Synonyms for: arm64-v8a, armeabi-v7a, x86_64, x86

  -r, --release
          Build a release build

      --no-cargo
          If the Rust library has been built for at least one target, then don't re-run cargo build.

          This may be useful if you are using a pre-built library or are managing the build process yourself.

  -g, --and-generate
          Optionally generate the bindings and turbo-module code for the crate

      --no-jniLibs
          Suppress the copying of the Rust library into the JNI library directories

  -h, --help
          Print help (see a summary with '-h')
```

`--release` sets the release profile for `cargo`.

`--and-generate` is a convenience option to pass the built library file to `generate bindings` and `generate turbo-module` for Android and common files.

This is useful as some generated files use the targets specified in this command.

Once the library files (one for each target) are created, they are copied into the `jniLibs` specified by the YAML configuration.

```admonish note
React Native requires that the Rust library be built as a static library. The CMake based build will combine the C++ with the static library into a shared object.

To configure Rust to build a static library, you should ensure `staticlib` is in the `crate-type` list in the `[lib]` section of the `Cargo.toml` file. Minimally, this should be in the `Cargo.toml` manifest file:

<pre>
<code class="hljs">[lib]
crate-type = ["staticlib"]
</code>
</pre>
```

We also need to make sure that we were linking to the correct NDK.

This changes from RN version to version, but in our usage we had to set an `ANDROID_NDK_HOME` variable in our script for this to pick up the appropriate version. For example:

```bash
export ANDROID_NDK_HOME=${ANDROID_SDK_ROOT}/ndk/26.1.10909125/
```

You can find the version you need in your react-native `android/build.gradle` file in the `ndkVersion` variable.

## `build ios`

Build the crate for use on an iOS device or simulator.
```
Build the crate for use on an iOS device or simulator

Usage: uniffi-bindgen-react-native build ios [OPTIONS] --config <CONFIG>

Options:
      --config <CONFIG>
          The configuration file for this build

      --sim-only
          Only build for the simulator

      --no-sim
          Exclude builds for the simulator

      --no-xcodebuild
          Does not perform the xcodebuild step to generate the xcframework

          The xcframework will need to be generated externally from this tool. This is useful when adding extra bindings (e.g. Swift) to the project.

  -t, --targets <TARGETS>...
          Comma separated list of targets, that override the values in the `config.yaml` file.

          iOS: aarch64-apple-ios, aarch64-apple-ios-sim, x86_64-apple-ios

  -r, --release
          Build a release build

      --no-cargo
          If the Rust library has been built for at least one target, then don't re-run cargo build.

          This may be useful if you are using a pre-built library or are managing the build process yourself.

  -g, --and-generate
          Optionally generate the bindings and turbo-module code for the crate

  -h, --help
          Print help (see a summary with '-h')
```

The configuration file refers to [the YAML configuration][config].

`--sim-only` and `--no-sim` restricts the targets to targets with/without `sim` in the target triple.

`--and-generate` is a convenience option to pass the built library file to `generate bindings` and `generate turbo-module` for iOS and common files.

This is useful as some generated files use the targets specified in this command.

Once the target libraries are compiled, and a config file is specified, they are passed to `xcodebuild -create-xcframework` to generate an `xcframework`.

```admonish note
React Native requires that the Rust library be built as a static library. The `xcodebuild` based build will combine the C++ with the static library `.xcframework` file.

To configure Rust to build a static library, you should ensure `staticlib` is in the `crate-type` list in the `[lib]` section of the `Cargo.toml` file. Minimally, this should be in the `Cargo.toml` manifest file:

<pre>
<code class="hljs">[lib]
crate-type = ["staticlib"]
</code>
</pre>
```

# `generate`

This command is to generate code for:

1. turbo-modules: installing the Rust crate into a running React Native app
2. bindings: the code needed to actually bridge between Javascript and the Rust library.

All subcommands require a [configuration file][config].

If you're already using `--and-generate`, then you don't need to know how to invoke this command.

```sh
Generate bindings or the turbo-module glue code from the Rust.

These steps are already performed when building with `--and-generate`.

Usage: uniffi-bindgen-react-native generate <COMMAND>

Commands:
  bindings      Generate just the Typescript and C++ bindings
  turbo-module  Generate the TurboModule code to plug the bindings into the app
  all           Generate the Bindings and TurboModule code from a library file and a YAML config file
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')
```

## `generate bindings`
Generate just the bindings. In most cases, this command should not be called directly, but with the build, with `--and-generate`.

```admonish info
This command follows the command line format of other `uniffi-bindgen` commands. Most arguments are passed straight to [`uniffi-bindgen::library_mode::generate_bindings`](https://docs.rs/uniffi_bindgen/0.28/uniffi_bindgen/library_mode/fn.generate_bindings.html).

For more/better documentation, please see the linked docs.
```

```admonish warning
Because this mirrors other `uniffi-bindgen`s, the `--config` option here is asking for a [`uniffi.toml`](uniffi-toml) file.
```

This command will generate two typescript files and two C++ files per Uniffi namespace. These are: `namespace.ts`, `namespace-ffi.ts`, `namespace.h`, `namespace.cpp`, substituting `namespace` for names derived from the Rust crate.

The [namespace is defined as](https://docs.rs/uniffi_bindgen/latest/uniffi_bindgen/interface/struct.ComponentInterface.html#method.namespace):

> The string namespace within which this API should be presented to the caller.
>
> This string would typically be used to prefix function names in the FFI, to build a package or module name for the foreign language, etc.

It may also be thought of as a crate or sub-crate which exports uniffi API.

The C++ files will be put into the `--cpp-dir` and the typescript files into the `--ts-dir`.

The C++ files can register themselves with the Hermes runtime.

```
Usage: uniffi-bindgen-react-native generate bindings [OPTIONS] --ts-dir <TS_DIR> --cpp-dir <CPP_DIR> <SOURCE>

Arguments:
  <SOURCE>
          A UDL file or library file

Options:
      --lib-file <LIB_FILE>
          The path to a dynamic library to attempt to extract the definitions from and extend the component interface with

      --crate <CRATE_NAME>
          Override the default crate name that is guessed from UDL file path.

          In library mode, this

      --config <CONFIG>
          The location of the uniffi.toml file

      --library
          Treat the input file as a library, extracting any Uniffi definitions from that

      --no-format
          By default, bindgen will attempt to format the code with prettier and clang-format

      --ts-dir <TS_DIR>
          The directory in which to put the generated Typescript

      --cpp-dir <CPP_DIR>
          The directory in which to put the generated C++

  -h, --help
          Print help (see a summary with '-h')
```
## `generate turbo-module`
Generate the TurboModule code to plug the bindings into the app.

More details about the files generated is shown [here](turbo-module-files.md).

```
Usage: uniffi-bindgen-react-native generate turbo-module --config <CONFIG> [NAMESPACES]...

Arguments:
  [NAMESPACES]...  The namespaces that are generated by `generate bindings`

Options:
      --config <CONFIG>  The configuration file for this build
  -h, --help             Print help
```

The namespaces in the commmand line are derived from the crate that has had its bindings created.

```admonish info
The locations of the files are derived from [the configuration file][config] and the project's package.json` file.

The relationships between files are preserved-- e.g. where one file points to another via a relative path, the relative path is calculated from these locations.
```

## `generate all`

This command performs the generation of both `bindings` and `turbo-module`, using a `lib.a` file.

This is a convenience method for users who do not or cannot use the `ubrn build` commands.

```sh
Generate the Bindings and TurboModule code from a library file and a YAML config file.

This is the second step of the `--and-generate` option of the build command.

Usage: uniffi-bindgen-react-native generate all --config <CONFIG> <LIB_FILE>

Arguments:
  <LIB_FILE>
          A path to staticlib file

Options:
      --config <CONFIG>
          The configuration file for this project

  -h, --help
          Print help (see a summary with '-h')
```

# `help`

Prints the help message.

```
Usage: uniffi-bindgen-react-native <COMMAND>

Commands:
  checkout  Checkout a given Github repo into `rust_modules`
  build     Build (and optionally generate code) for Android or iOS
  generate  Generate bindings or the turbo-module glue code from the Rust
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

You can add `--help` to any command to get more information about that command.
