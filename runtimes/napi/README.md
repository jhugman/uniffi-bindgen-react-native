# uniffi-runtime-napi

Call Rust libraries from Node.js — without writing any glue code by hand.

## What it does

You have a Rust library compiled as a `.dylib` or `.so`. You want to call its functions from JavaScript. Normally, you'd write C bindings, or use a code generator to produce a native addon for each library.

uniffi-runtime-napi takes a different path. It loads your Rust library at runtime, looks up its functions by name, and calls them directly. You describe the function signatures in JavaScript, and uniffi-runtime-napi handles the rest: marshalling values across the boundary, managing memory, and routing callbacks between threads.

## Why this approach

UniFFI uses a small, fixed set of FFI types — integers, floats, byte buffers, callbacks, structs. That set doesn't change no matter which Rust crate you're wrapping. So instead of generating a new native addon for each library, you can build one addon that handles all the types, and drive it with data.

This has three benefits:

- **Faster development** — change a type description in JS, reload. No Rust recompile for the binding layer.
- **Simpler packaging** — one prebuilt native addon works with any UniFFI library. Ship the `.dylib` and the generated JS, nothing else.
- **Easier to add new targets** — writing a bindgen for Bun or Deno means generating JS that talks to the same uniffi-runtime-napi addon.

The approach is inspired by [node-ffi-rs](https://github.com/zhangyuang/node-ffi-rs), which proved that runtime FFI from Node.js works well in practice. uniffi-runtime-napi narrows the scope to UniFFI's calling conventions, which lets it handle things ffi-rs couldn't — struct-by-value passing, cross-thread callback dispatch, and `BigInt` for 64-bit values.

This is the FFI backend for [uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native). A code generator produces the JavaScript type descriptions; uniffi-runtime-napi executes them.

## How it works

Three steps:

**1. Open a library**

```js
const mod = UniffiNativeModule.open("/path/to/libfoo.dylib");
```

This calls `dlopen` under the hood.

**2. Register your functions**

```js
const nm = mod.register({
  symbols: {
    rustbufferAlloc: "uniffi_foo_rustbuffer_alloc",
    rustbufferFree: "uniffi_foo_rustbuffer_free",
    rustbufferFromBytes: "uniffi_foo_rustbuffer_from_bytes",
  },
  structs: {},
  callbacks: {},
  functions: {
    uniffi_foo_fn_add: {
      args: [FfiType.Int32, FfiType.Int32],
      ret: FfiType.Int32,
      hasRustCallStatus: true,
    },
  },
});
```

For each function, uniffi-runtime-napi looks up the symbol, builds a call descriptor using [libffi](https://sourceware.org/libffi/) (a C library that calls functions whose signatures aren't known until runtime), and returns a callable JavaScript function.

**3. Call them**

```js
const status = { code: 0 };
const result = nm.uniffi_foo_fn_add(3, 4, status);
// result === 7
```

Arguments go in as JavaScript values, come out as JavaScript values. Errors from Rust land in the `status` object.

## What crosses the boundary

| JS side | Rust side |
|---------|-----------|
| `number` | `i8`–`i32`, `u8`–`u32`, `f32`, `f64` |
| `BigInt` | `i64`, `u64`, object handles |
| `Uint8Array` | `RustBuffer` (byte buffers) |
| `Function` | callback pointers |
| `Object` | VTable structs (trait implementations) |

## Callbacks and threads

Rust code can call back into JavaScript. uniffi-runtime-napi handles two cases:

- **Same thread**: calls the JS function directly (fast).
- **Different thread**: serializes the arguments, dispatches to the main thread, blocks until the JS function returns.

This matters for async Rust code that runs work on background threads but needs to call foreign trait methods defined in JS.

## Building

```sh
# Build the native addon
npm run build

# Run tests (builds test fixtures first)
cd fixtures/test_lib && cargo build && cd ../..
cd fixtures/uniffi-fixture-simple && cargo build && cd ../..
npm test
```

Requires Rust, Node.js, and a C compiler (for libffi).

## Project structure

```
src/
  lib.rs          — entry point, library open/close
  register.rs     — parses definitions, builds CIFs, creates JS functions
  ffi_type.rs     — the type system: every FFI type as an enum
  cif.rs          — maps types to libffi descriptors
  marshal.rs      — converts values between JS and C
  callback.rs     — callback trampolines (same-thread + cross-thread)
  structs.rs      — VTable struct building for foreign traits
  fn_pointer.rs   — wraps C function pointers as callable JS functions
  library.rs      — dlopen/dlsym wrapper
fixtures/         — test libraries written in Rust
tests/            — Node.js test suite
docs/             — API reference and design notes
```

## Status

Early development. Scalars, buffers, callbacks, cross-thread dispatch, VTables, and async Rust futures all work and are tested.
