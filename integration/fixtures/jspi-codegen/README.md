# Generated JSPI integration fixture

This fixture pins wasm-bindgen 0.2.128 and builds UniFFI
scaffolding, generated Rust wrappers and generated TypeScript APIs. The imports
return controllable synthetic Promises. No external service is used.

The CI job `Integration tests (JSPI web and wasm2)` runs this suite together with
the wasm2 suites and shared runtime tests using Node 24.14.0. To reproduce the
combined run after installing the repository, `typescript`, and `runtimes/wasm`
JavaScript dependencies and the Rust WASM target:

```sh
UBRN_WASM_BINDGEN=/tmp/ubrn-jspi-tools/bin/wasm-bindgen sh scripts/run-jspi-tests.sh
```

The runner rebuilds the CLI, runtimes and generated fixtures, checks their
TypeScript, and stops on the first failure. The matching wasm-bindgen must be
installed first as shown below. Browser checks remain manual.

From the repository root, with the wasm32-unknown-unknown Rust target installed:

```sh
cargo build -p uniffi-bindgen-react-native
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root /tmp/ubrn-jspi-tools
UBRN_WASM_BINDGEN=/tmp/ubrn-jspi-tools/bin/wasm-bindgen sh integration/fixtures/jspi-codegen/build.sh
node --experimental-wasm-jspi --experimental-wasm-exnref integration/fixtures/jspi-codegen/run.mjs
```

The build script runs strict TypeScript checking and bundles the shared test with
the repository's esbuild (installed with the JS development dependencies). It
supports native library paths on macOS and Linux. Set `RUSTC_WRAPPER=`
if a locally configured compiler cache is unavailable.

For the same tests in a JSPI-capable browser:

```sh
python3 -m http.server 8766 --bind 127.0.0.1 --directory integration/fixtures/jspi-codegen
```

Open `http://127.0.0.1:8766/`. Both runners should display
`36 generated JSPI cases passed`. Without Node's JSPI flag, loading the generated
module must fail with the explicit “These bindings require WebAssembly JSPI”
message before glue evaluation.

Coverage includes scalar, byte, Unicode string, record and void results;
throwing and non-throwing signatures; caught import rejection converted into
UniFFI errors; repeated suspension; two calls settling out of order; memory
growth; event-loop progress; an unchanged synchronous function; and a function
selected by both `forceAsync` and `jspi`. Type annotations in the test check the
public return signatures, including a compile-time rejection of `new Processor`.
The primary JSPI factory is `Processor.create`, matching the async
constructor convention. Runtime status ownership tests also live in
`typescript/tests/rust-call.test.ts`.

Object coverage includes primary and alternate factories, failure after partial
construction, suspending Display, object method errors and void calls, two methods
settling out of order after explicit destruction, object arguments kept alive
through suspension, and async use scopes releasing on success and rejection. A
Rust drop counter verifies exact release timing. Record, flat-enum and tagged-enum
methods suspend; unselected object constructors and methods stay synchronous.

Rust async coverage includes owned byte results, void and error results,
Pending/wake/repoll, overlap and memory growth, pre-abort, cancellation during
suspension, successful owned results winning cancellation, async constructors,
and object lifetimes across async methods. Unselected Rust async APIs still work.
The runtime unit tests cover both poll signal orderings and fatal poll failure
containment without intentionally trapping a live Rust stack.

Lifecycle tests use explicit destruction for deterministic release assertions.
Destructors stay synchronous. Fatal traps are mocked in runtime unit tests;
integration tests use recoverable Rust errors.
