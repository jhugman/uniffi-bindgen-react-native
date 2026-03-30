# uniffi-runtime-napi API Reference

uniffi-runtime-napi is a napi-rs native addon that provides a low-level FFI bridge between Node.js and Rust libraries built with [UniFFI](https://github.com/nickel-lang/uniffi-rs). It replaces ffi-rs with proper support for BigInt, struct-by-value, callbacks, and cross-thread dispatch.

This document is the reference for **code generators** (e.g. uniffi-bindgen-node) that emit JavaScript/TypeScript code targeting uniffi-runtime-napi.

## Quick Example

```javascript
import lib from "uniffi-runtime-napi/lib.js";
const { UniffiNativeModule, FfiType } = lib;

// 1. Open the compiled Rust library
const mod = UniffiNativeModule.open("./target/debug/libmylib.dylib");

// 2. Register all FFI definitions at once (including per-crate RustBuffer symbols)
const nativeModule = mod.register({
  symbols: {
    rustbufferAlloc: "uniffi_mylib_rustbuffer_alloc",
    rustbufferFree: "uniffi_mylib_rustbuffer_free",
    rustbufferFromBytes: "uniffi_mylib_rustbuffer_from_bytes",
  },
  structs: {
    /* VTable definitions */
  },
  callbacks: {
    /* callback signatures */
  },
  functions: {
    /* all exported functions */
  },
});

// 3. Call functions by name — they are pre-compiled, no per-call overhead
const status = { code: 0 };
const result = nativeModule.uniffi_mylib_fn_add(3, 4, status);
```

---

## API

### `UniffiNativeModule.open(path)`

Opens a native library via `dlopen`.

| Parameter | Type     | Description                                            |
| --------- | -------- | ------------------------------------------------------ |
| `path`    | `string` | Absolute or relative path to the `.dylib` / `.so` file |

**Returns:** `UniffiNativeModule` instance.

**Throws:** If the library cannot be opened.

In a megazord (multi-crate) setup, multiple crates share the same library path. Call `open()` once and call `register()` per crate:

```javascript
const lib = UniffiNativeModule.open(megazordPath);
const crate1 = lib.register({ symbols: crate1Symbols, ...crate1Defs });
const crate2 = lib.register({ symbols: crate2Symbols, ...crate2Defs });
```

### `module.register(definitions)`

Resolves per-crate RustBuffer symbols and pre-compiles all FFI definitions into callable JavaScript functions. This is called once per crate at module initialization. At call time, there are no string lookups or type resolution.

```typescript
const nativeModule = module.register({
  symbols: {
    rustbufferAlloc: 'uniffi_{crate}_rustbuffer_alloc',
    rustbufferFree: 'uniffi_{crate}_rustbuffer_free',
    rustbufferFromBytes: 'uniffi_{crate}_rustbuffer_from_bytes',
  },
  structs: { ... },
  callbacks: { ... },
  functions: { ... },
});
```

**Returns:** A plain JavaScript object where each key is a function name from `definitions.functions`, and each value is a callable function.

**Throws:** If any RustBuffer symbol cannot be found in the library.

The RustBuffer symbols follow UniFFI's naming convention:

```
uniffi_{crate_name}_rustbuffer_alloc
uniffi_{crate_name}_rustbuffer_free
uniffi_{crate_name}_rustbuffer_from_bytes
```

The sections of the definitions object are described below.

---

## `definitions.functions`

Maps function symbol names to their FFI signatures.

```javascript
functions: {
  symbol_name: {
    args: [FfiType.*, ...],     // Array of argument types
    ret: FfiType.*,             // Return type
    hasRustCallStatus: boolean, // Whether the C function takes a trailing &mut RustCallStatus
  },
}
```

### Calling Convention

Each registered function takes **positional arguments** matching its `args` array. If `hasRustCallStatus` is `true`, the **last** JavaScript argument must be a `{ code: number }` status object.

```javascript
// Registered as:
//   uniffi_foo_fn_bar: { args: [FfiType.Int32, FfiType.RustBuffer], ret: FfiType.Handle, hasRustCallStatus: true }

// Called as:
const callStatus = { code: 0 };
const result = nativeModule.uniffi_foo_fn_bar(42, someUint8Array, callStatus);
// result is a BigInt (Handle type)
// callStatus.code is written back after the call
// callStatus.errorBuf is set to a Uint8Array if code != 0
```

The C-level signature is: `uint64_t uniffi_foo_fn_bar(int32_t, RustBuffer, RustCallStatus*)`. The `RustCallStatus*` is invisible to the JS caller — uniffi-runtime-napi handles it internally based on `hasRustCallStatus`.

### RustCallStatus Writeback

When `hasRustCallStatus` is `true`:

1. Before the C call: `code` is read from the JS object and written to a stack-allocated C `RustCallStatus`
2. The C struct pointer is appended as the final C argument
3. After the C call: `code` is written back to the JS object
4. If `code != 0` and the C function populated `error_buf`: the error buffer bytes are copied into a `Uint8Array` and set as `callStatus.errorBuf`, then the C buffer is freed via `rustbuffer_free`

---

## `definitions.callbacks`

Maps callback names to their C-level signatures. These definitions are referenced by `FfiType.Callback(name)` in function args and by `FfiType.Callback(name)` fields in struct definitions.

```javascript
callbacks: {
  callback_name: {
    args: [FfiType.*, ...],     // C argument types (excluding RustCallStatus)
    ret: FfiType.*,             // C return type
    hasRustCallStatus: boolean, // Whether the C signature includes a trailing &mut RustCallStatus
  },
}
```

Callback definitions are **schemas only** — they describe the C function pointer signature. The actual JavaScript functions are provided later, either:

- As function arguments when calling a registered function (for per-call callbacks)
- As properties on a JavaScript object when passing a VTable struct

### Callback Dispatch

UniFFI does not support passing closures across the FFI. All user-facing callbacks are delivered through **VTable structs** (foreign trait vtables). The only non-VTable function pointers are internal ones generated by UniFFI's scaffolding (e.g. async continuation callbacks).

When C code invokes a callback trampoline:

**Main thread:** The JS function is called directly and synchronously. Return values and RustCallStatus writeback work normally.

**Other thread:** The dispatch mode is determined implicitly by the callback's own properties:

| Condition                                    | Mode             | Behavior                                                                                        |
| -------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------- |
| `ret != Void` or `hasRustCallStatus: true`   | **Blocking**     | Calling thread blocks until JS completes; return value and RustCallStatus sent back via channel |
| `ret == Void` and `hasRustCallStatus: false` | **Non-blocking** | Fire-and-forget; calling thread does not wait                                                   |

User callbacks (foreign trait methods) always return a value or have RustCallStatus, so they use blocking dispatch. Internal scaffolding callbacks (async continuations, free) have void return and no RustCallStatus, so they fire-and-forget.

No explicit flag is needed — the dispatch mode falls out naturally from `ret` and `hasRustCallStatus`.

### Callback RustCallStatus

When a callback has `hasRustCallStatus: true`, the C-level signature includes an extra `&mut RustCallStatus` parameter beyond the declared `args`. The trampoline:

1. Creates a JS `{ code }` object from the C status
2. Passes it as the last argument to the JS function
3. After the JS function returns, writes `code` back from the JS object to the C struct

This allows the JS callback to signal errors back to Rust.

---

## `definitions.structs`

Maps struct names to their field definitions. Currently used for **VTable structs** (foreign trait vtables where each field is a callback function pointer).

```javascript
structs: {
  StructName: [
    { name: 'field_name', type: FfiType.Callback('callback_name') },
    { name: 'another_field', type: FfiType.Callback('other_callback') },
  ],
}
```

Each field has:
| Property | Type | Description |
|----------|------|-------------|
| `name` | `string` | Field name (must match the C struct field order) |
| `type` | `FfiType` | Field type — typically `FfiType.Callback(name)` for VTables |

### VTable Passing

When a registered function has an argument of type `FfiType.Reference(FfiType.Struct('StructName'))`, the caller passes a plain JavaScript object whose properties match the struct field names:

```javascript
// Registered as:
//   uniffi_foo_fn_init_callback_vtable: {
//     args: [FfiType.Reference(FfiType.Struct('VTable_MyTrait'))],
//     ret: FfiType.Void,
//     hasRustCallStatus: true,
//   }

// Called as:
nativeModule.uniffi_foo_fn_init_callback_vtable(
  {
    do_thing: (handle, arg1, callStatus) => {
      callStatus.code = 0;
      return someValue;
    },
    uniffi_free: (handle, callStatus) => {
      callStatus.code = 0;
    },
  },
  callStatus,
);
```

For each `Callback` field, uniffi-runtime-napi:

1. Creates a persistent reference to the JS function (prevents GC)
2. Allocates a C trampoline via `ffi_closure_alloc`
3. Builds the C struct with function pointers in field order
4. Passes a pointer to the struct to the C function

VTable trampolines are **long-lived** — they persist for the lifetime of the `UniffiNativeModule`. This is correct because VTable init functions are typically called once at startup, and Rust holds the function pointers indefinitely.

---

## FfiType Reference

`FfiType` is exported from `lib.js`. Simple types are plain objects with a `tag` property. Parameterized types are factory functions.

### Scalar Types

| FfiType           | JS Argument Type | JS Return Type | C Type     | Notes                          |
| ----------------- | ---------------- | -------------- | ---------- | ------------------------------ |
| `FfiType.UInt8`   | `number`         | `number`       | `uint8_t`  |                                |
| `FfiType.Int8`    | `number`         | `number`       | `int8_t`   |                                |
| `FfiType.UInt16`  | `number`         | `number`       | `uint16_t` |                                |
| `FfiType.Int16`   | `number`         | `number`       | `int16_t`  |                                |
| `FfiType.UInt32`  | `number`         | `number`       | `uint32_t` |                                |
| `FfiType.Int32`   | `number`         | `number`       | `int32_t`  |                                |
| `FfiType.UInt64`  | `bigint`         | `bigint`       | `uint64_t` | BigInt required                |
| `FfiType.Int64`   | `bigint`         | `bigint`       | `int64_t`  | BigInt required                |
| `FfiType.Float32` | `number`         | `number`       | `float`    |                                |
| `FfiType.Float64` | `number`         | `number`       | `double`   |                                |
| `FfiType.Handle`  | `bigint`         | `bigint`       | `uint64_t` | Object handle; BigInt required |
| `FfiType.Void`    | —                | `undefined`    | `void`     | Only valid as return type      |

### Compound Types

| FfiType                  | JS Argument Type | JS Return Type | C Type                  | Notes                                                                 |
| ------------------------ | ---------------- | -------------- | ----------------------- | --------------------------------------------------------------------- |
| `FfiType.RustBuffer`     | `Uint8Array`     | `Uint8Array`   | `RustBuffer` (by value) | Serialized data; copied via `rustbuffer_from_bytes`/`rustbuffer_free` |
| `FfiType.RustCallStatus` | —                | —              | `RustCallStatus*`       | Not used in `args`/`ret`; handled by `hasRustCallStatus` flag         |
| `FfiType.ForeignBytes`   | —                | —              | `ForeignBytes`          | Internal; not directly used in register definitions                   |
| `FfiType.VoidPointer`    | —                | —              | `void*`                 | Opaque pointer                                                        |

### Parameterized Types

| Factory                       | Creates                          | Usage                                                                                   |
| ----------------------------- | -------------------------------- | --------------------------------------------------------------------------------------- |
| `FfiType.Callback(name)`      | `{ tag: 'Callback', name }`      | Function argument: pass a JS function. Struct field: callback function pointer slot.    |
| `FfiType.Struct(name)`        | `{ tag: 'Struct', name }`        | References a struct defined in `definitions.structs`                                    |
| `FfiType.Reference(inner)`    | `{ tag: 'Reference', inner }`    | Pointer to inner type. Used as `Reference(Struct('Name'))` for VTable pass-by-reference |
| `FfiType.MutReference(inner)` | `{ tag: 'MutReference', inner }` | Mutable pointer to inner type                                                           |

---

## Mapping from UniFFI's `FfiType`

This table shows how each variant of UniFFI's `FfiType` enum (from `uniffi_bindgen::interface::FfiType`) maps to a uniffi-runtime-napi `FfiType` constant.

| UniFFI `FfiType`               | uniffi-runtime-napi `FfiType`         | Notes                                  |
| ------------------------------ | ----------------------------- | -------------------------------------- |
| `FfiType::UInt8`               | `FfiType.UInt8`               |                                        |
| `FfiType::Int8`                | `FfiType.Int8`                |                                        |
| `FfiType::UInt16`              | `FfiType.UInt16`              |                                        |
| `FfiType::Int16`               | `FfiType.Int16`               |                                        |
| `FfiType::UInt32`              | `FfiType.UInt32`              |                                        |
| `FfiType::Int32`               | `FfiType.Int32`               |                                        |
| `FfiType::UInt64`              | `FfiType.UInt64`              |                                        |
| `FfiType::Int64`               | `FfiType.Int64`               |                                        |
| `FfiType::Float32`             | `FfiType.Float32`             |                                        |
| `FfiType::Float64`             | `FfiType.Float64`             |                                        |
| `FfiType::Handle`              | `FfiType.Handle`              | Object handles (pointers as u64)       |
| `FfiType::RustBuffer`          | `FfiType.RustBuffer`          | Serialized as `Uint8Array`             |
| `FfiType::ForeignBytes`        | `FfiType.ForeignBytes`        |                                        |
| `FfiType::RustCallStatus`      | _(not used directly)_         | Controlled by `hasRustCallStatus` flag |
| `FfiType::Callback(name)`      | `FfiType.Callback(name)`      |                                        |
| `FfiType::Struct(name)`        | `FfiType.Struct(name)`        |                                        |
| `FfiType::Reference(inner)`    | `FfiType.Reference(inner)`    | Recursive                              |
| `FfiType::MutReference(inner)` | `FfiType.MutReference(inner)` | Recursive                              |
| `FfiType::VoidPointer`         | `FfiType.VoidPointer`         |                                        |

---

## Code Generation Guide

This section describes how a code generator (e.g. uniffi-bindgen-node) should emit code targeting uniffi-runtime-napi.

### Step 1: Generate the `open()` call

Generate the library open call. In a megazord, this is shared across crates:

```javascript
import lib from "uniffi-runtime-napi/lib.js";
const { UniffiNativeModule, FfiType } = lib;

const mod = UniffiNativeModule.open(libraryPath);
```

### Step 2: Generate the `register()` call

From the UniFFI component interface, extract the crate name for the RustBuffer symbols. Then iterate `ci.ffi_definitions()` to build the definition maps:

#### Structs

For each `FfiDefinition::Struct` (typically VTables for callback interfaces / foreign traits):

```javascript
structs: {
  // struct.name() -> field list
  'VTable_CallbackInterface_MyTrait': [
    // For each field: { name, type }
    { name: 'do_thing', type: FfiType.Callback('callback_my_trait_do_thing') },
    { name: 'uniffi_free', type: FfiType.Callback('callback_my_trait_free') },
  ],
},
```

#### Callbacks

For each `FfiDefinition::CallbackFunction`:

```javascript
callbacks: {
  // callback.name() -> { args, ret, hasRustCallStatus }
  'callback_my_trait_do_thing': {
    args: [FfiType.Handle, FfiType.Int32],  // from callback.arguments()
    ret: FfiType.Void,                       // from callback.return_type()
    hasRustCallStatus: true,                 // from callback.has_rust_call_status()
  },
},
```

#### Functions

For each `FfiDefinition::Function`:

```javascript
functions: {
  // function.name() -> { args, ret, hasRustCallStatus }
  'uniffi_{crate}_fn_method_myobj_do_thing': {
    args: [FfiType.Handle, FfiType.Int32, FfiType.RustBuffer],
    ret: FfiType.RustBuffer,
    hasRustCallStatus: true,
  },
},
```

**Important:** The `args` array must **not** include `RustCallStatus`. That parameter is implicit — controlled by `hasRustCallStatus`. Include only the "real" arguments.

### Step 3: Generate wrapper functions

The object returned by `register()` has untyped functions. Generate typed wrappers:

```typescript
// Generated from UniFFI interface definition
export function myObj_doThing(
  handle: bigint,
  arg1: number,
  buf: Uint8Array,
): Uint8Array {
  const callStatus = { code: 0 };
  const result = nativeModule.uniffi_mylib_fn_method_myobj_do_thing(
    handle,
    arg1,
    buf,
    callStatus,
  );
  // Check callStatus, handle errors via the shared ubrn runtime
  return result;
}
```

### Step 4: Generate VTable initialization

For each callback interface or foreign trait, generate a VTable init call:

```typescript
// Called once at module load time
const vtable = {
  do_thing: (handle: bigint, arg1: number, callStatus: { code: number }) => {
    // Dispatch to the registered JS implementation
    callStatus.code = 0;
    return result;
  },
  uniffi_free: (handle: bigint, callStatus: { code: number }) => {
    // Clean up the JS-side handle
    callStatus.code = 0;
  },
};

const status = { code: 0 };
nativeModule.uniffi_mylib_fn_init_callback_vtable_mytrait(vtable, status);
```

---

## Supported Patterns

| Pattern                               | Supported | Notes                                                                          |
| ------------------------------------- | --------- | ------------------------------------------------------------------------------ |
| Scalar function calls                 | Yes       | All integer, float, and void types                                             |
| BigInt for 64-bit values              | Yes       | `UInt64`, `Int64`, `Handle` use `bigint`                                       |
| RustBuffer (serialized data)          | Yes       | `Uint8Array` ↔ C `RustBuffer` by value                                         |
| RustCallStatus error propagation      | Yes       | `code` + `errorBuf` written back to JS                                         |
| Callback function arguments           | Yes       | Trampolines via `ffi_closure_alloc`; internal scaffolding callbacks only       |
| Cross-thread callbacks (blocking)     | Yes       | User callbacks with return values or RustCallStatus; blocks until JS completes |
| Cross-thread callbacks (non-blocking) | Yes       | Fire-and-forget for internal scaffolding callbacks (continuations, free)       |
| VTable structs (foreign traits)       | Yes       | Struct of callback function pointers, passed by reference                      |
| Callback return values                | Yes       | VTable callbacks can return scalars, including cross-thread                    |
| Async (Rust futures)                  | Planned   | Requires `uniffiRustCallAsync` integration with continuation callbacks         |
