#!/bin/sh
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.
set -eu
cd "$(dirname "$0")/.."

# Keep the fixture toolchain separate from the workspace's wasm-bindgen.
bindgen=${UBRN_WASM_BINDGEN:-wasm-bindgen}
if [ "$("$bindgen" --version)" != "wasm-bindgen 0.2.128" ]; then
    echo "Set UBRN_WASM_BINDGEN to wasm-bindgen 0.2.128." >&2
    exit 1
fi
node --experimental-wasm-jspi --experimental-wasm-exnref --input-type=module -e '
if (typeof WebAssembly.promising !== "function" || typeof WebAssembly.Suspending !== "function") {
    throw new Error("JSPI tests require a Node build with JSPI enabled (tested on 24.14.0)");
}'

cargo build --locked -p uniffi-bindgen-react-native
npm --prefix typescript run build
npm --prefix runtimes/wasm run build
npm --prefix runtimes/wasm test
node_modules/.bin/tsx typescript/tests/rust-call.test.ts
node_modules/.bin/tsx typescript/tests/async-rust-call.test.ts

# Each build includes strict TypeScript checking of freshly generated bindings.
sh integration/fixtures/jspi-codegen/build.sh
node --experimental-wasm-jspi --experimental-wasm-exnref integration/fixtures/jspi-codegen/run.mjs
sh integration/fixtures/jspi-wasm2-codegen/build.sh
for suite in run-generated run-lifecycle; do
    node --experimental-wasm-jspi --experimental-wasm-exnref "integration/fixtures/jspi-wasm2-codegen/$suite.mjs"
done
