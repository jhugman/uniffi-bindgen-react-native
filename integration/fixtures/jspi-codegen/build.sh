#!/bin/sh
set -eu
cd "$(dirname "$0")"
bindgen=${UBRN_WASM_BINDGEN:-wasm-bindgen}
if [ "$("$bindgen" --version)" != "wasm-bindgen 0.2.128" ]; then
    echo "Set UBRN_WASM_BINDGEN to wasm-bindgen 0.2.128." >&2
    exit 1
fi
cargo build --locked --target-dir target
case "$(uname -s)" in
    Darwin) library=target/debug/libjspi_codegen.dylib ;;
    Linux) library=target/debug/libjspi_codegen.so ;;
    *) echo "Use the equivalent native library path on this platform." >&2; exit 1 ;;
esac
../../../target/debug/uniffi-bindgen-react-native generate wasm bindings \
    --library --ts-dir generated/ts --cpp-dir generated/rs --no-format "$library"
cargo build --locked --release --features bindings --target wasm32-unknown-unknown --target-dir target
"$bindgen" target/wasm32-unknown-unknown/release/jspi_codegen.wasm \
    --target web --out-dir generated/ts/wasm-bindgen --out-name index
../../../node_modules/.bin/tsc -p tsconfig.json
../../../node_modules/.bin/esbuild test.ts --bundle --format=esm --outfile=generated/test.js --tsconfig=tsconfig.json
