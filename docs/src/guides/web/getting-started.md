# Getting started with WASM

This extends the [Step-by-step tutorial with React Native](../rn/getting-started.md). We've split this out running the library under WASM involves creating an expo app.

## Preparing the library

Add a script to the `package.json`, if you haven't already:

```diff
    "script": {
+     "ubrn:web": "ubrn web build"
    },
```

Also, add the entrypoint for `browser`s:

```diff
+  "browser": "src/index.web.ts",
+  "react-native": "src/index.tsx",
```

You can ensure that the bindings get generated specifically for both react-native and the web, by changing the `ubrn.config.yaml` file.

```diff
rust:
  repo: https://github.com/jhugman/uniffi-starter.git
  branch: jhugman/bump-uniffi-to-0.29
  manifestPath: rust/foobar/Cargo.toml
+ web:
+   ts: src/generated/web
+ bindings:
+   ts: src/generated/rn
```

Once these changes are done, then you can run:

```sh
yarn ubrn:web
```

This does a number of things, but you end up with:

- an entrypoint file `src/index.web.ts`
- a bindings file called `src/generated/web/foobar.rs`
- some wasm-bindgen generated files in `src/generated/web/wasm-bindgen`:
  - `index_bg.wasm`
  - `index.js`
  - `index.d.ts`
  - `index_bg.wasm.d.ts`

Now, you should be ready to write an example app.

## Making the example app

I'm going to make use `expo` to make an example app in the directory next to our `my-rust-lib` directory.

```sh
export dir=my-wasm-app
yarn \
    create \
    expo-app \
    --template blank-typescript \
    --yes \
    $dir
cd $dir
```

We'll need to install the `react-native-web` libraries, and associated bits that converts the React Native JSX to Web JSX, which in turn converts to a sea of divs.

```sh
npx expo install \
    react-dom \
    react-native-web \
    @expo/metro-runtime
```

Then, add our `my-rust-lib` library.

```sh
yarn add ../my-rust-lib
```

```admonish warning title="Help wanted"
I don't really understand how npm and yarn do linking or workspaces.

Doing `yarn add ../my-rust-lib` copies everything into the `node_modules` directory of the example, which is less than ideal.

If you know a better way, please open a PR. Help!
```

### Write a demo

```diff
import { StyleSheet, View, Text } from 'react-native';
-import { multiply } from 'react-native-my-rust-lib';
-
-const result = multiply(3, 7);

export default function App() {
```

Next, add the following lines in place of the lines we just deleted:

```ts
import { Calculator, type BinaryOperator, SafeAddition, ComputationResult } from 'my-rust-lib';

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

### Initializing the WASM in the web page

Next, we need to update the timing of App registration.

We need to edit `example/input.js`:

```diff
import { AppRegistry } from 'react-native';
import App from './src/App';
import { name as appName } from './app.json';
+import { uniffiInitAsync } from "my-rust-lib";

+uniffiInitAsync().then(() => {
+   AppRegistry.registerComponent(appName, () => App);
+});
- AppRegistry.registerComponent(appName, () => App);
```

You may also initialize the WASM in a `useEffect` block.

### Teaching Metro about WASM files

You may have to show your bundler what to do with WASM file—just serve them as binary data files.

```js
// Learn more https://docs.expo.io/guides/customizing-metro
const { getDefaultConfig } = require('expo/metro-config');

/** @type {import('expo/metro-config').MetroConfig} */
const config = getDefaultConfig(__dirname);

// Add wasm asset support
config.resolver.assetExts.push('wasm');

// Add COEP and COOP headers to support SharedArrayBuffer
config.server.enhanceMiddleware = (middleware) => {
  return (req, res, next) => {
    res.setHeader('Cross-Origin-Embedder-Policy', 'credentialless');
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    middleware(req, res, next);
  };
};

module.exports = config;
```

The operative part of this configuration is marking `wasm` files as assets.

```js
config.resolver.assetExts.push('wasm');
```

### Running in a page

Running `yarn web` should now open a web page showing the result to be 42.

## Getting started without React Native

Outside of the React Native ecosystem, the important two steps are:

- asynchronously initializing the WASM bundle, using `uniffiInitAsync`
- getting your bundler to allow asynchronous serving of WASM files.

### For Webpack

The [wasm example in the webpack repository](https://github.com/webpack/webpack/blob/main/examples/wasm-simple/webpack.config.js) is instructive here.

The operative step is to set `experiments.asyncWebAssembly = true` in your current WASM config.

```admonish warning title="Help wanted"
I'm really not a real web developer, so would very much appreciate help with this documentation from someone who is.
```
