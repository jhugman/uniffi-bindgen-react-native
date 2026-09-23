#!/bin/sh
set -eu
cd "$(dirname "$0")"
bindgen=${UBRN_WASM_BINDGEN:-wasm-bindgen}
if [ "$("$bindgen" --version)" != "wasm-bindgen 0.2.128" ]; then
    echo "Set UBRN_WASM_BINDGEN to wasm-bindgen 0.2.128." >&2
    exit 1
fi
cargo build --locked --lib --release --target wasm32-unknown-unknown --target-dir target
../../../target/debug/uniffi-bindgen-react-native generate wasm2 bindings \
    --library --ts-dir generated/api --no-format target/wasm32-unknown-unknown/release/jspi_wasm2_codegen.wasm
UBRN_WASM_BINDGEN="$bindgen" cargo run --locked --bin jspi-wasm2-codegen --target-dir target
../../../node_modules/.bin/tsc -p tsconfig.json
../../../node_modules/.bin/esbuild generated-test.ts --bundle --format=esm --outfile=generated/generated-test.js --tsconfig=tsconfig.json
../../../node_modules/.bin/esbuild lifecycle-test.ts --bundle --format=esm --outfile=generated/lifecycle-test.js --tsconfig=tsconfig.json
