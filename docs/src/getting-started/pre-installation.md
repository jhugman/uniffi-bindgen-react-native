# Before you start

Better resources are available than this site for installing these dependencies.

Below are a list of the dependencies, and a non-comprehensive instructions on how to get them onto your system.

## Install Rust
If Rust isn't already installed on your system, you should install it as per the [rust-lang.org install instructions](https://www.rust-lang.org/tools/install).

This will add `cargo` and `rustup` to your path, which are the main entry points into Rust.

## Install C++ tooling

These commands will add the tooling needed to compile and run the generated C++ code.

Optionally, `clang-format` can be installed to format the generated C++ code.

For MacOS, using homebrew:
```sh
brew install cmake ninja clang-format
```

For Debian flavoured Linux:
```sh
apt-get install cmake ninja clang-format
```

For generared Typescript, the existing `prettier` installation is detected and your configuration is used.

## Android

### Add the Android specific targets

This command adds the backends for the Rust compiler to emit machine code for different Android architectures.

```sh
rustup target add \
    aarch64-linux-android \
    armv7-linux-androideabi \
    i686-linux-android \
    x86_64-linux-android
```

### Install `cargo-ndk`

> This cargo extension handles all the environment configuration needed for successfully building libraries for Android from a Rust codebase, with support for generating the correct jniLibs directory structure.

```sh
cargo install cargo-ndk
```

## iOS

### Ensure `xcodebuild` is avaiable

This command checks if Xcode command line tools are available, and if not, will start the installation process.

```sh
xcode-select --install
```

### Add the iOS specific targets

This command adds the backends for the Rust compiler to emit machine code for different iOS architectures.

```sh
rustup target add \
    aarch64-apple-ios \
    aarch64-apple-ios-sim \
    x86_64-apple-ios
```
