# Generated wasm2 JSPI integration tests

This fixture tests generated TypeScript bindings through the wasm2 player and
staging pipeline. It uses synthetic Promise-returning imports, UniFFI 0.31.2
and wasm-bindgen 0.2.128.

## Running

Install the repository, `typescript`, and `runtimes/wasm` JavaScript dependencies,
the `wasm32-unknown-unknown` Rust target, and wasm-bindgen CLI 0.2.128. From the
repository root:

```sh
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root /tmp/ubrn-jspi-tools
UBRN_WASM_BINDGEN=/tmp/ubrn-jspi-tools/bin/wasm-bindgen sh scripts/run-jspi-tests.sh
```

The same runner is used by the `Integration tests (JSPI web and wasm2)` CI job
with Node 24.14.0. It builds the CLI and runtimes, runs runtime tests, regenerates
both backends' bindings, checks strict TypeScript, and runs their Node suites.
Set `RUSTC_WRAPPER=` if a locally configured compiler cache is unavailable.

To build and run only this fixture:

```sh
cargo build -p uniffi-bindgen-react-native
UBRN_WASM_BINDGEN=/tmp/ubrn-jspi-tools/bin/wasm-bindgen sh integration/fixtures/jspi-wasm2-codegen/build.sh
node --experimental-wasm-jspi --experimental-wasm-exnref integration/fixtures/jspi-wasm2-codegen/run-generated.mjs
node --experimental-wasm-jspi --experimental-wasm-exnref integration/fixtures/jspi-wasm2-codegen/run-lifecycle.mjs
```

For browser tests, serve this directory:

```sh
python3 -m http.server 8771 --bind 127.0.0.1 --directory integration/fixtures/jspi-wasm2-codegen
```

Open `http://127.0.0.1:8771/generated.html` for the 28 function/async cases and
`http://127.0.0.1:8771/lifecycle.html` for the 27 lifecycle cases. Browser runs
require JSPI support. Node runners enforce a 15-second deadline.

## Behavior covered

`generated-test.ts` covers numeric, string, byte, record and void results;
multiple large owned arguments; errors; overlapping and repeated suspension;
memory growth; stable returned buffers; and unselected synchronous/async APIs.
Async cases cover owned results, Pending/wake/repoll, cancellation before and
during polling, completion winning cancellation, and deterministic future drops.

`lifecycle-test.ts` covers primary, alternate and async factories; partial
construction failures; Display; method errors and void calls; overlapping
methods finishing after explicit destruction; object arguments; async use
scopes; record/enum methods; cancellation; and unselected objects. Drop counters
verify release timing, and strict TypeScript checks factory and enum signatures.

The staging assertions check that the cargo artifact is preserved and that glue
and declarations are emitted beside the bindings without a shadowing JS entry.
Runtime unit tests cover frame ownership, fatal failure containment, and thunk
encoding. Fatal traps are mocked; the integration tests use recoverable Rust
errors. Lifecycle tests use explicit destruction rather than GC scheduling.
