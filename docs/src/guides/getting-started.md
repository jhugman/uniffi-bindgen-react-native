# Step-by-step tutorial

This tutorial will get you started, by taking an existing Rust crate, and building a React Native library from it.

By the end of this tutorial you will:

1. have a working turbo-module library,
1. an example app, running in both Android and iOS,
1. seen how to set up `uniffi-bindgen-react-native` for your library.

## Step 1: Start with builder-bob

We first use `create-react-native-library` to generate our basic turbo-module library.

```sh
npx create-react-native-library@latest my-rust-lib
```

```admonish warning title="Builder Bob version drift"
`create-react-native-library` has changed a few things around recently.

These steps have been tested with `0.35.1` and `0.41.2`, which at time of writing, is the `latest`.
```

The important bits are:
```
✔ What type of library do you want to develop? › Turbo module
✔ Which languages do you want to use? › C++ for Android & iOS
✔ What type of example app do you want to create? › Vanilla
```

For following along, here are the rest of my answers.

```
✔ What is the name of the npm package? … react-native-my-rust-lib
✔ What is the description for the package? … My first React Native library in Rust
✔ What is the name of package author? … James Hugman
✔ What is the email address for the package author? … james@nospam.fm
✔ What is the URL for the package author? … https://github.com/jhugman
✔ What is the URL for the repository? … https://github.com/jhugman/react-native-my-rust-lib
✔ What type of library do you want to develop? › Turbo module
✔ Which languages do you want to use? › C++ for Android & iOS
✔ What type of example app do you want to create? › Vanilla
✔ Project created successfully at my-rust-lib!
```

Most of the rest of the command line guide will be done within the directory this has just created.

```sh
cd my-rust-lib
```

Verify everything works before adding Rust:

For iOS:

```sh
yarn
(cd example/ios && pod install)
yarn example start
```

Then `i` for iOS.

After that has launched, then you can hit the `a` key to launch Android.

We should, if all has gone to plan, see `Result: 21` on screen.

## Step 2: Add `uniffi-bindgen-react-native` to the project

Using `yarn` add the `uniffi-bindgen-react-native` package to your project.

```sh
yarn add uniffi-bindgen-react-native
```

```admonish warning title="Pre-release"
While this is before the first release, we're installing straight from github.

`yarn add uniffi-bindgen-react-native@https://github.com/jhugman/uniffi-bindgen-react-native`
```

Opening `package.json` add the following:

```diff
  "scripts": {
+    "ubrn:ios":      "ubrn build ios     --config ubrn.config.yaml --and-generate && (cd example/ios && pod install)",
+    "ubrn:android":  "ubrn build android --config ubrn.config.yaml --and-generate",
+    "ubrn:checkout": "ubrn checkout      --config ubrn.config.yaml",
+    "ubrn:clean": "rm -Rf cpp/ android/src/main/java ios/ src/Native* src/generated/ src/index.ts*",
    "example": "yarn workspace react-native-my-rust-lib-example",
    "test": "jest",
    "typecheck": "tsc",
    "lint": "eslint \"**/*.{js,ts,tsx}\"",
    "clean": "del-cli android/build example/android/build example/android/app/build example/ios/build lib",
    "prepare": "bob build",
    "release": "release-it"
  },
```

You can call the config file whatever you want, I have called it `ubrn.config.yaml` in this example.

For now, let's just clean the files out we don't need:

```sh
yarn ubrn:clean
```

```admonish hint
If you're going to be using the `uniffi-bindgen-react-native` command direct from the command line, it may be worth setting up an alias. In bash you can do this:
```

```sh
alias ubrn=$(yarn ubrn --path)
```

There is a guide to the `ubrn` command [here][cli].

[cli]: ../reference/commandline.md


```admonish warning title="Pre-release"
While this is before the first release, we're installing straight from local `node_modules`.

After release, the C++ runtime will be published to Cocoa Pods.

Until then, you need to add the dependency to the app's Podfile, in this case `example/ios/Podfile`:
```

```diff
  use_react_native!(
    :path => config[:reactNativePath],
    # An absolute path to your application root.
    :app_path => "#{Pod::Config.instance.installation_root}/.."
  )

+  # We need to specify this here in the app because we can't add a local dependency within
+  # the react-native-matrix-rust-sdk
+  pod 'uniffi-bindgen-react-native', :path => '../../node_modules/uniffi-bindgen-react-native'
```

## Step 3: Create the `ubrn.config.yaml` file

Full documentation on how to configure your library can be found in [the YAML configuration file page][config] of this book.

[config]: ../reference/config-yaml.md

For now, we just want to get started; let's start with an existing Rust crate that has uniffi bindings.

```yaml
---
name: MyRustLib
rust:
  repo: https://github.com/ianthetechie/uniffi-starter
  branch: main
  manifestPath: rust/foobar/Cargo.toml
```

## Step 4: Checkout the Rust code

Now, you should be able to checkout the Rust into the library.

```sh
yarn ubrn:checkout
```

This will checkout the `uniffi-starter` repo into the `rust_modules` directory within your project.

You may want to add to `.gitignore` at this point:

```diff
+# From uniffi-bindgen-react-native
+rust_modules/
+*.a
```

## Step 4: Build the Rust

Building for iOS will:

1. Build the Rust crate for iOS, including the uniffi scaffolding in Rust.
1. Build an `xcframework` for Xcode to pick up.
1. Generate the typescript and C++ bindings between Hermes and the Rust.
1. Generate the files to set up the JS -> Objective C -> C++ installation flow for the turbo-module.
1. Re-run the `Podfile` in the `example/ios` directory so Xcode can see the C++ files.

```sh
yarn ubrn:ios
```

Building for Android will:

1. Build the Rust crate for Android, including the uniffi scaffolding in Rust.
1. Copy the files into the correct place in for `gradlew` to pick them up.
1. Generate the files to set up the JS -> Java -> C++ installation flow for the turbo-module.
1. Generate the files to make a turbo-module from the C++.

```sh
yarn ubrn:android
```

```admonish hint
You can change the targets that get built by adding a comma separated list to the `ubrn build android` and `ubrn build ios` commands.
```

```sh
yarn ubrn:android --targets aarch64-linux-android,armv7-linux-androideabi
```

```admonish warning title="Troubleshooting"
This won't happen with the `uniffi-starter` library, however a common error is to not enable a `staticlib` crate type in the project's `Cargo.toml`. Instructions on how to do this are given [here](../reference/commandline.md#admonition-note).
```

## Step 5: Write an example app exercising the Rust API

Here, we're editing the app file at `example/src/App.tsx`.

First we delete the starter code given to us by `create-react-native-library`:

```diff
import { StyleSheet, View, Text } from 'react-native';
-import { multiply } from 'react-native-my-rust-lib';
-
-const result = multiply(3, 7);

export default function App() {
```

Next, add the following lines in place of the lines we just deleted:

```ts
import { Calculator, type BinaryOperator, SafeAddition, ComputationResult } from '../../src';

// A Rust object
const calculator = new Calculator();
// A Rust object implementing the Rust trait BinaryOperator
const addOp = new SafeAddition();

// A Typescript class, implementing BinaryOperator
class SafeMultiply implements BinaryOperator {
  perform(lhs: bigint, rhs: bigint): bigint {
    return lhs * rhs;
  }
}
const multOp = new SafeMultiply();

// bigints
const three = 3n;
const seven = 7n;

// Perform the calculation, and to get an object
// representing the computation result.
const computation: ComputationResult = calculator
  .calculate(addOp, three, three)
  .calculateMore(multOp, seven)
  .lastResult()!;

// Unpack the bigint value into a string.
const result = computation.value.toString();
```

## Step 6: Run the example app

Now you can run the apps on Android and iOS:

```sh
yarn example start
```

As with the starter app from `create-react-native-library`, there is very little to look at.

We should, if all has gone to plan, see `Result: 42` on screen.

## Step 7: Make changes in the Rust

We can edit the Rust, in this case in `rust_modules/uniffi-starter/rust/foobar/src/lib.rs`.

If you're already familiar with Rust, you will notice that there is very little unusual about this file, apart from a few `uniffi` proc macros scattered here or there.

If you're not familiar with Rust, you might add a function to the Rust:

```rust
#[uniffi::export]
pub fn greet(who: String) -> String {
  format!("Hello, {who}!")
}
```

Then run either `yarn ubrn:ios` or `yarn ubrn:android`.

Once either of those are run, you should be able to import the `greet` function into `App.tsx`.

# Appendix: the Rust

The Rust library is presented here for comparison with the `App.tsx` above.

All credit should go to the author, [ianthetechie][ianthetechie].

[ianthetechie]: https://github.com/ianthetechie/

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
// You must call this once
uniffi::setup_scaffolding!();

// What follows is an intentionally ridiculous whirlwind tour of how you'd expose a complex API to UniFFI.

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum ComputationState {
    /// Initial state with no value computed
    Init,
    Computed {
        result: ComputationResult
    },
}

#[derive(Copy, Clone, Debug, PartialEq, uniffi::Record)]
pub struct ComputationResult {
    pub value: i64,
    pub computation_time: Duration,
}

#[derive(Debug, PartialEq, thiserror::Error, uniffi::Error)]
pub enum ComputationError {
    #[error("Division by zero is not allowed.")]
    DivisionByZero,
    #[error("Result overflowed the numeric type bounds.")]
    Overflow,
    #[error("There is no existing computation state, so you cannot perform this operation.")]
    IllegalComputationWithInitState,
}

/// A binary operator that performs some mathematical operation with two numbers.
#[uniffi::export(with_foreign)]
pub trait BinaryOperator: Send + Sync {
    fn perform(&self, lhs: i64, rhs: i64) -> Result<i64, ComputationError>;
}

/// A somewhat silly demonstration of functional core/imperative shell in the form of a calculator with arbitrary operators.
///
/// Operations return a new calculator with updated internal state reflecting the computation.
#[derive(PartialEq, Debug, uniffi::Object)]
pub struct Calculator {
    state: ComputationState,
}

#[uniffi::export]
impl Calculator {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            state: ComputationState::Init
        }
    }

    pub fn last_result(&self) -> Option<ComputationResult> {
        match self.state {
            ComputationState::Init => None,
            ComputationState::Computed { result } => Some(result)
        }
    }

    /// Performs a calculation using the supplied binary operator and operands.
    pub fn calculate(&self, op: Arc<dyn BinaryOperator>, lhs: i64, rhs: i64) -> Result<Calculator, ComputationError> {
        let start = Instant::now();
        let value = op.perform(lhs, rhs)?;

        Ok(Calculator {
            state: ComputationState::Computed {
                result: ComputationResult {
                    value,
                    computation_time: start.elapsed()
                }
            }
        })
    }

    /// Performs a calculation using the supplied binary operator, the last computation result, and the supplied operand.
    ///
    /// The supplied operand will be the right-hand side in the mathematical operation.
    pub fn calculate_more(&self, op: Arc<dyn BinaryOperator>, rhs: i64) -> Result<Calculator, ComputationError> {
        let ComputationState::Computed { result } = &self.state else {
            return Err(ComputationError::IllegalComputationWithInitState);
        };

        let start = Instant::now();
        let value = op.perform(result.value, rhs)?;

        Ok(Calculator {
            state: ComputationState::Computed {
                result: ComputationResult {
                    value,
                    computation_time: start.elapsed()
                }
            }
        })
    }
}

#[derive(uniffi::Object)]
struct SafeAddition {}

// Makes it easy to construct from foreign code
#[uniffi::export]
impl SafeAddition {
    #[uniffi::constructor]
    fn new() -> Self {
        SafeAddition {}
    }
}

#[uniffi::export]
impl BinaryOperator for SafeAddition {
    fn perform(&self, lhs: i64, rhs: i64) -> Result<i64, ComputationError> {
        lhs.checked_add(rhs).ok_or(ComputationError::Overflow)
    }
}

```
