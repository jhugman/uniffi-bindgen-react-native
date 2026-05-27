# Node.js (N-API) support

As of `0.31.0-3`, `uniffi-bindgen-react-native` can generate TypeScript bindings that run directly on Node.js, in addition to React Native and the Web.

This support is new and is currently scoped to generating bindings and running them against an already-compiled Rust library. Unlike the React Native and Web targets, there is not yet an end-to-end `build` command that compiles Rust, scaffolds a project, and wires everything together. If you are comfortable building your own `cdylib` and calling `ubrn generate napi`, this page describes what is available today.

## How it works

Node.js support has two pieces:

- **The bindgen** generates TypeScript that describes your library's FFI surface — the function signatures, structs, and callbacks that cross the boundary.
- **The runtime**, published to npm as [`@ubjs/node`](https://www.npmjs.com/package/@ubjs/node), loads your compiled Rust `cdylib` at runtime, looks up its functions by name, and calls into them using [libffi](https://sourceware.org/libffi/).

Because UniFFI uses a small, fixed set of FFI types, a single prebuilt native addon (`@ubjs/node`) works with any UniFFI library — there is no per-library glue code to compile. The generated TypeScript drives that addon with data.

The TypeScript runtime helpers (FFI converters, the `RustBuffer` type, and so on) are shared with the other targets and are published as [`@ubjs/core`](https://www.npmjs.com/package/@ubjs/core).

## Generating bindings

Bindings are generated with the `napi` subcommand of `ubrn generate` (the `node` alias also works):

```sh
ubrn generate napi bindings \
  --library path/to/libmy_crate.dylib \
  --ts-dir path/to/generated/ts \
  --lib-colocated
```

| Option | Description |
| ------ | ----------- |
| `--library <PATH>` | The compiled Rust `cdylib` to read the FFI metadata from. |
| `--ts-dir <DIR>` | The directory the generated TypeScript is written to. |
| `--no-format` | Skip formatting the generated code with `prettier` (which is run by default). |

One of the three library-resolution modes below is required. It controls how the generated TypeScript locates the `cdylib` at runtime.

### Library resolution

The generated code needs to know where to find the `cdylib` when it runs. The mode you choose is baked into the bindings.

#### `--lib-colocated`

The binary sits next to the generated `.js` file at runtime. This is the simplest mode and is well suited to local development.

```sh
ubrn generate napi bindings --library ./libmy_crate.dylib --ts-dir ./ts --lib-colocated
```

#### `--lib-absolute`

The absolute path of `--library` is baked into the bindings as an override. The `--library` path must be absolute.

```sh
ubrn generate napi bindings --library /abs/path/libmy_crate.dylib --ts-dir ./ts --lib-absolute
```

#### `--lib-package-base <BASE>`

The `cdylib` is resolved via platform-specific npm packages using `require.resolve`, the same pattern used to distribute prebuilt native binaries on npm. Requires `--library` so the crate name can be derived.

The package name is formed from `BASE` and a target triple. If `BASE` ends in an alphanumeric character, a `-` separator is appended; otherwise the trailing punctuation is used as the literal separator:

| `BASE` | Resolved package |
| ------ | ---------------- |
| `@scope/foo` | `@scope/foo-<triple>` |
| `@scope/foo/` | `@scope/foo/<triple>` |
| `@scope/foo_` | `@scope/foo_<triple>` |

By default the triples are cargo-style (e.g. `aarch64-apple-darwin`). Pass `--lib-node-triple` to emit node-style triples instead (e.g. `darwin-arm64`, `linux-x64-gnu`, `win32-x64-msvc`):

```sh
ubrn generate napi bindings \
  --library ./libmy_crate.dylib --ts-dir ./ts \
  --lib-package-base @scope/foo --lib-node-triple
```

`--lib-node-triple` has no effect without `--lib-package-base`, and is rejected when combined with `--lib-colocated` or `--lib-absolute`.

## Running the bindings

Add the runtime packages to your project and run the generated TypeScript on Node.js:

```sh
npm install @ubjs/node @ubjs/core
```

The generated bindings import from `@ubjs/core` and use `@ubjs/node` to open and call into your `cdylib`. With `--lib-colocated`, place the compiled `cdylib` next to the generated JavaScript.

## Limitations

- No `ubrn build node` / scaffolding command yet — you compile the `cdylib` and run `ubrn generate napi` yourself.
- C++ bindings are not generated for this target; only TypeScript is produced.

See the [`@ubjs/node` README](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/runtimes/napi) for lower-level details on how the runtime marshals values, dispatches callbacks across threads, and loads libraries.
