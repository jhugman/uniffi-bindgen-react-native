# The player `DEFINITIONS` table

This page is the reference for the table the bindgen hands to a player runtime. Read it before changing the type vocabulary, adding a flavor, or writing a runtime that consumes the table. For how one runtime uses it, see [The `wasm2` player](wasm2-player.md).

## What it is

A player is a runtime that calls any UniFFI `cdylib` without code generated for that crate. It can do so because the signatures it needs are data: for each FFI export, the bindgen emits one entry naming the argument types and the return type, and the player builds a callable from it at load time.

Three flavors are players and they share one table:

| Flavor | Runtime | What builds the call |
| ------ | ------- | -------------------- |
| `napi` | `@ubjs/node`, Rust over libffi | `runtimes/core` |
| `wasm2` | `@ubjs/wasm`, pure TypeScript | `runtimes/wasm/core` |
| `jsi2` | a C++ shim over `runtimes/core`, installed as `globalThis.uniffi` | `runtimes/core` |

The bindgen emits the table into `<namespace>-ffi.ts` as a constant named `DEFINITIONS`. The table body is identical across the three flavors; what differs is the import that supplies the `FfiType` factory and the call that registers the table. See [Per-flavor differences](#per-flavor-differences).

## The root object

```typescript
interface ModuleDefinitions {
  symbols: {
    rustbuffer_alloc: string;
    rustbuffer_free: string;
    rustbuffer_from_bytes: string;
  };
  functions: Record<string, FunctionDef>;
  callbacks: Record<string, CallbackDef>;
  structs: Record<string, FieldDesc[]>;
}
```

`symbols` names the three `RustBuffer` lifecycle exports of the crate, `uniffi_<crate>_rustbuffer_alloc` and friends. `napi` and `jsi2` resolve them from the library and use them to allocate and free buffers. `wasm2` validates that the first two are non-empty and then ignores them: it allocates through the helper crate's `__ubrn_alloc` and `__ubrn_free`, which reach the same Rust global allocator without a per-crate symbol lookup.

The raw export symbol name keys `functions`; the UniFFI name of the function type or struct keys `callbacks` and `structs`. Entries in `functions`, `callbacks` and `structs` refer to one another only by those names, so a table is self-contained.

## The type vocabulary

Every argument, return and field is an `FfiTypeDesc`: a tagged object built with the `FfiType` factory.

```typescript
type FfiTypeDesc =
  | { tag: "UInt8" } | { tag: "Int8" }
  | { tag: "UInt16" } | { tag: "Int16" }
  | { tag: "UInt32" } | { tag: "Int32" }
  | { tag: "UInt64" } | { tag: "Int64" }
  | { tag: "Float32" } | { tag: "Float64" }
  | { tag: "Handle" }
  | { tag: "RustBuffer" }
  | { tag: "ForeignBytes" }
  | { tag: "RustCallStatus" }
  | { tag: "VoidPointer" }
  | { tag: "Void" }
  | { tag: "Callback"; name: string }
  | { tag: "Struct"; name: string }
  | { tag: "Reference"; inner: FfiTypeDesc }
  | { tag: "MutReference"; inner: FfiTypeDesc };
```

The tags are UniFFI's own `FfiType` variants, and the bindgen maps them one to one in [`type_mapping.rs`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/gen_typescript/ffi_module_player/type_mapping.rs). A runtime that meets an unknown tag throws at registration, not at the first call.

Each tag fixes both the C type and the JavaScript value that crosses the boundary.

| Tag | C type | JavaScript value |
| --- | ------ | ---------------- |
| `UInt8` … `Int32` | the matching integer | `number` |
| `UInt64`, `Int64` | 64-bit integer | `bigint` |
| `Float32`, `Float64` | `float`, `double` | `number` |
| `Handle` | `u64` index into a Rust-side object registry | `bigint` |
| `RustBuffer` | the `(capacity, len, dataPtr)` struct, by value | `Uint8Array` |
| `Void` | no value | `undefined`; return position only |
| `Callback(name)` | function pointer whose signature is `callbacks[name]` | a JavaScript function |
| `Struct(name)` | the struct `structs[name]`, by value | a plain object keyed by field name |
| `Reference(inner)`, `MutReference(inner)` | pointer to `inner` | as for `inner` |
| `VoidPointer` | `void*` | pointer-sized integer |
| `RustCallStatus` | the status struct | see [Call status](#call-status) |
| `ForeignBytes` | a `(len, dataPtr)` pair | never emitted |

Three of those need more than a row.

**`Reference` only ever wraps a vtable struct.** The bindgen emits `Reference(Struct("VTable…"))` for the vtable argument of each `init_callback_vtable_*` function, and nothing else. All three runtimes marshal only that shape: `wasm2` and `jsi2` reject any other inner type at registration, and `napi` builds the vtable outside its generic scalar path. Each allocates the struct once and never frees it, because Rust keeps the pointer for the life of the library.

**`RustCallStatus` never appears in `args`.** A function that takes a trailing status pointer says so with `hasRustCallStatus: true`, and the runtime adds the parameter. The tag appears only as a struct field, `call_status` in the `ForeignFutureResult*` structs.

**`ForeignBytes` is in the vocabulary but not in any table.** The only export that takes one is `rustbuffer_from_bytes`, which the bindgen lists under `symbols` rather than `functions`. Both `napi` and `wasm2` reject it if asked.

## `functions`

```typescript
interface FunctionDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;          // FfiType.Void when the export returns nothing
  hasRustCallStatus: boolean;
}
```

Registration turns each entry into a JavaScript function on the returned module object, under the same key. The function takes one JavaScript value per element of `args` and, when `hasRustCallStatus` is true, one more: a status object the caller supplies. It returns the lifted `ret`, or `undefined` when the status reports an error.

```typescript
uniffi_uniffi_futures_fn_method_megaphone_say_after: {
  args: [FfiType.Handle, FfiType.UInt16, FfiType.RustBuffer],
  ret: FfiType.Handle,
  hasRustCallStatus: false,
},
ffi_uniffi_futures_rust_future_complete_u64: {
  args: [FfiType.Handle],
  ret: FfiType.Handle,
  hasRustCallStatus: true,
},
```

The bindgen lists every scaffolding function, object clone and free, vtable init, the contract-version export and every checksum. It lists the `rust_future_poll`, `complete`, `cancel` and `free` family only when the crate has an async function. It never lists the `rustbuffer_*` exports; those live in `symbols`. The filter is `should_include_function` in [`ffi_module/builder.rs`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/gen_typescript/ffi_module/builder.rs).

### Call status

The status object is the one `@ubjs/core` defines:

```typescript
type UniffiRustCallStatus = { code: number; errorBuf?: Uint8Array };
```

The caller passes `{ code: 0 }`. The runtime zeroes a real `RustCallStatus` for the call and, afterwards, copies `code` back and, when Rust set an error buffer, copies its bytes into a fresh `Uint8Array` at `errorBuf` and frees the Rust allocation. The generated code then throws from that buffer.

## `callbacks`

```typescript
interface CallbackDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
  outReturn?: boolean;       // absent means false
}
```

A callback entry describes a function Rust calls into JavaScript: the shape a JavaScript function must have before the generated code can pass it as a `Callback(name)`, whether as an argument or as a vtable field. The names come from UniFFI: `RustFutureContinuationCallback`, `CallbackInterface<Interface>Method<N>`, `CallbackInterfaceFree<Interface>`, `CallbackInterfaceClone<Interface>`, `ForeignFutureComplete<Type>`, and `ForeignFutureDroppedCallback`.

The JavaScript function receives one lifted value per element of `args`, using the same table as above. What it returns depends on the two flags.

**Neither flag set.** The function returns the `ret` value directly, or nothing for `Void`. Free, clone and the continuation callback look like this.

**`outReturn`.** UniFFI's vtable methods return through an out-pointer rather than a C return value: the C signature has a trailing `uniffi_out_return` (or `uniffi_out_dropped_callback`) pointer argument and returns `void`. The bindgen strips that argument from `args`, puts its pointee type in `ret`, and sets the flag. The runtime adds the pointer parameter back and writes the function's return value through it. The JavaScript side sees an ordinary return.

**`hasRustCallStatus`.** The runtime adds a status pointer parameter. The JavaScript function then returns a `UniffiResult`: on success `{ pointee: value }`, on failure `{ code, errorBuf }`, where a nonzero `code` makes the runtime skip the return write and copy the status back to Rust. This is the shape `@ubjs/core` already builds for synchronous callback-interface methods, so the generated code needs no adapter.

```typescript
CallbackInterfaceFuturesAsyncParserMethod1: {
  args: [
    FfiType.Handle,
    FfiType.Int32,
    FfiType.RustBuffer,
    FfiType.Callback("ForeignFutureCompletei32"),
    FfiType.Handle,
  ],
  ret: FfiType.Struct("ForeignFutureDroppedCallbackStruct"),
  hasRustCallStatus: false,
  outReturn: true,
},
ForeignFutureCompletei32: {
  args: [FfiType.Handle, FfiType.Struct("ForeignFutureResultI32")],
  ret: FfiType.Void,
  hasRustCallStatus: false,
},
```

The first entry is an async callback-interface method: it takes a completion callback and returns, through the out-pointer, the struct Rust will call to drop the future. The second is that completion callback, which takes the result struct by value.

```admonish note
A `Callback` argument inside a callback, such as the completion callback above, arrives as a JavaScript function too. The runtime wraps the function pointer Rust passed in a closure that lowers its arguments and calls back through the same table. Nesting is therefore free to the generated code, at the cost of a table lookup per level.
```

## `structs`

```typescript
interface FieldDesc { name: string; type: FfiTypeDesc }
// structs: Record<string, FieldDesc[]>
```

A struct is an ordered list of named fields. The runtime lays it out by the platform's C rules, so the order must match the Rust `#[repr(C)]` definition, which it does because both come from UniFFI's metadata. In JavaScript a struct is a plain object with one property per field.

Three families appear.

```typescript
VTableCallbackInterfaceFuturesAsyncParser: [
  { name: "uniffi_free",  type: FfiType.Callback("CallbackInterfaceFreeFutures_AsyncParser") },
  { name: "uniffi_clone", type: FfiType.Callback("CallbackInterfaceCloneFutures_AsyncParser") },
  { name: "as_string",    type: FfiType.Callback("CallbackInterfaceFuturesAsyncParserMethod0") },
  // one field per method …
],
ForeignFutureResultI32: [
  { name: "return_value", type: FfiType.Int32 },
  { name: "call_status",  type: FfiType.RustCallStatus },
],
ForeignFutureDroppedCallbackStruct: [
  { name: "handle", type: FfiType.Handle },
  { name: "free",   type: FfiType.Callback("ForeignFutureDroppedCallback") },
],
```

**Vtables** hold only callbacks. The generated code builds one object per callback interface at module scope, with a function per field, and passes it by `Reference` to the vtable init export. Instance identity travels as the `Handle` first argument of each method, which is why one vtable serves every object of the interface.

**Foreign future results** carry a value and a status by value into a `ForeignFutureComplete*` callback. `ForeignFutureResultVoid` has the status field alone.

**The dropped-callback struct** is what an async callback method hands back to Rust through its out-pointer: a handle and the function Rust calls to release it.

## Per-flavor differences

The table body is the same text in all three flavors. The template, [`wrapper-ffi-player.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/gen_typescript/templates/wrapper-ffi-player.ts), switches only the framing.

| | `napi` | `wasm2` | `jsi2` |
| - | ------ | ------- | ------ |
| `FfiType` comes from | `@ubjs/node` default export | `@ubjs/wasm/core` | `@ubjs/core` |
| Type check on the literal | none | `satisfies ModuleDefinitions` | `as const` only |
| Registered by | `UniffiNativeModule.open(path).register(DEFINITIONS)` in the getter | `registerSync(PLAYER_DEFINITIONS)` from the generated `index.ts` | `globalThis.uniffi.open(path).register(DEFINITIONS)` in the getter |
| Parsed by | `runtimes/napi/src/register/spec_from_js.rs` | `registerSync` in `runtimes/wasm/core/src/module.ts` | `cpp/jsi-player-shim/shim.cpp`, into a `#[repr(C)]` spec for `runtimes/core` |
| Missing `callbacks` or `structs` | tolerated | error | tolerated |

Two of those rows matter when you edit the vocabulary.

`@ubjs/node` ships its `FfiType` factory as plain JavaScript with no declaration file, so a `napi` table is not type-checked at all. A misspelled tag surfaces when `spec_from_js.rs` rejects it at load time. Only `wasm2` catches the mistake at compile time.

The `wasm2` runtime treats a `Callback` argument with `outReturn` or `hasRustCallStatus` as unsupported when it appears *inside* a callback. Vtable methods come from the table's `callbacks` map and support both flags; the restriction covers only function pointers Rust passes in, none of which UniFFI generates with either flag today.

## Where the schema is declared

The schema has no single source. Each consumer declares it in its own language, and the bindgen renders it as strings.

| Path | Language | Role |
| ---- | -------- | ---- |
| `typescript/src/ffi-definitions.ts` | TypeScript | `FfiTypeDesc`, the `FfiType` factory, `ModuleDefinitions`, `FunctionDef`, `CallbackDef`, `FieldDesc`; `@ubjs/wasm/core` re-exports them |
| `runtimes/napi/lib.js` | JavaScript | the `FfiType` factory, no types |
| `runtimes/core/src/ffi_type.rs`, `spec.rs` | Rust | `FfiTypeDesc`, `ModuleSpec` and the three `*Def` structs |
| `runtimes/napi/src/register/spec_from_js.rs` | Rust | the parser from the JavaScript object to `ModuleSpec` |
| `crates/ubrn_bindgen/src/bindings/gen_typescript/ffi_module_player/` | Rust | the IR the template renders; `args` and `ret` are already strings such as `"FfiType.UInt32"` |
| `crates/ubrn_bindgen/tests/player_template_snapshots.rs`, `wasm2_codegen.rs` | Rust | snapshots that pin the emitted shape |

```admonish warning
Adding a tag means changing all of them: the two factories, the TypeScript union in `@ubjs/core`, the Rust enum, the `napi` parser, the `wasm2` `planArg` and lift switches, the bindgen mapping, and the snapshots. Nothing checks that the set is the same in every place except the fixture tests, which only exercise tags UniFFI emits.
```
