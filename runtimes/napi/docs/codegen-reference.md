# uniffi-runtime-napi Code Generation Reference

> **Audience:** This document is for coding agents and code generators that emit JavaScript/TypeScript targeting uniffi-runtime-napi. It provides exact patterns, exhaustive type mappings, and copy-pasteable templates.

## Overview

uniffi-runtime-napi exposes two methods: `open()` and `register()`. Your generated code calls `open()` once to load a Rust `.dylib`/`.so`, then `register()` once per crate with a definitions object containing the crate's RustBuffer symbols and all its FFI definitions (functions, callbacks, structs). `register()` returns a plain JS object with pre-compiled callable functions. In a megazord setup, one `open()` call is shared across multiple `register()` calls.

**Import pattern:**

```javascript
import lib from "uniffi-runtime-napi/lib.js";
const { UniffiNativeModule, FfiType } = lib;
```

---

## Complete FfiType Mapping

### From UniFFI `FfiType` enum to JS `FfiType` constant

Use this table to convert `uniffi_bindgen::interface::FfiType` variants into JavaScript `FfiType` expressions:

```
FfiType::UInt8           →  FfiType.UInt8
FfiType::Int8            →  FfiType.Int8
FfiType::UInt16          →  FfiType.UInt16
FfiType::Int16           →  FfiType.Int16
FfiType::UInt32          →  FfiType.UInt32
FfiType::Int32           →  FfiType.Int32
FfiType::UInt64          →  FfiType.UInt64
FfiType::Int64           →  FfiType.Int64
FfiType::Float32         →  FfiType.Float32
FfiType::Float64         →  FfiType.Float64
FfiType::Handle          →  FfiType.Handle
FfiType::RustBuffer      →  FfiType.RustBuffer
FfiType::ForeignBytes    →  FfiType.ForeignBytes
FfiType::VoidPointer     →  FfiType.VoidPointer
FfiType::Callback(name)  →  FfiType.Callback('name')
FfiType::Struct(name)    →  FfiType.Struct('name')
FfiType::Reference(inner)    →  FfiType.Reference(<inner>)
FfiType::MutReference(inner) →  FfiType.MutReference(<inner>)
```

For `Reference` and `MutReference`, apply the mapping recursively to `inner`.

**`FfiType::RustCallStatus` is never emitted directly.** It is handled implicitly by the `hasRustCallStatus` boolean on each function/callback definition.

**`FfiType::Void` is only valid as a return type:** `FfiType.Void`

### JS Value Types at Call Sites

When calling a registered function, each argument must be the correct JS type:

```
FfiType.UInt8        →  number
FfiType.Int8         →  number
FfiType.UInt16       →  number
FfiType.Int16        →  number
FfiType.UInt32       →  number
FfiType.Int32        →  number
FfiType.UInt64       →  bigint    ← NOT number
FfiType.Int64        →  bigint    ← NOT number
FfiType.Float32      →  number
FfiType.Float64      →  number
FfiType.Handle       →  bigint    ← NOT number
FfiType.RustBuffer   →  Uint8Array
FfiType.Callback(n)  →  Function  (for per-call callbacks)
                     →  ——        (not passed directly for VTable fields)
FfiType.Reference(FfiType.Struct(n)) → Object  (with properties matching struct fields)
FfiType.Void         →  (return only: undefined)
```

Return values follow the same type mapping. `FfiType.Void` returns `undefined`.

---

## The `register()` Definitions Object

### Structure

```javascript
module.register({
  symbols: {   // Per-crate RustBuffer management symbols
    rustbufferAlloc: 'uniffi_{crate}_rustbuffer_alloc',
    rustbufferFree: 'uniffi_{crate}_rustbuffer_free',
    rustbufferFromBytes: 'uniffi_{crate}_rustbuffer_from_bytes',
  },
  structs: {   // Map<string, Array<FieldDef>>
    ...
  },
  callbacks: { // Map<string, CallbackDef>
    ...
  },
  functions: { // Map<string, FunctionDef>
    ...
  },
})
```

All four keys are required (`symbols` must have all three symbol names; use `{}` for empty maps on the others).

### `functions` — Map of exported C functions

Each key is the exact C symbol name. Each value is:

```javascript
{
  args: FfiType[],        // Argument types, IN ORDER, excluding RustCallStatus
  ret: FfiType,           // Return type
  hasRustCallStatus: bool // true if C signature has trailing &mut RustCallStatus
}
```

**Rules:**

- `args` must NOT include the RustCallStatus parameter — it is implicit
- The order of `args` must match the C function's parameter order
- Symbol names must be the exact exported C symbol (e.g. `uniffi_mylib_fn_method_foo_bar`)

**Example — scalar function:**

```javascript
uniffi_mylib_fn_add: {
  args: [FfiType.Int32, FfiType.Int32],
  ret: FfiType.Int32,
  hasRustCallStatus: true,
}
// C: int32_t uniffi_mylib_fn_add(int32_t, int32_t, RustCallStatus*)
// JS: nativeModule.uniffi_mylib_fn_add(3, 4, callStatus) → number
```

**Example — function taking RustBuffer and returning Handle:**

```javascript
uniffi_mylib_fn_method_foo_bar: {
  args: [FfiType.Handle, FfiType.RustBuffer],
  ret: FfiType.Handle,
  hasRustCallStatus: true,
}
// C: uint64_t fn(uint64_t, RustBuffer, RustCallStatus*)
// JS: nativeModule.uniffi_mylib_fn_method_foo_bar(42n, uint8Array, callStatus) → bigint
```

**Example — void function with no args:**

```javascript
uniffi_mylib_fn_do_nothing: {
  args: [],
  ret: FfiType.Void,
  hasRustCallStatus: true,
}
// JS: nativeModule.uniffi_mylib_fn_do_nothing(callStatus) → undefined
```

**Example — function taking a callback argument:**

```javascript
uniffi_mylib_fn_poll_future: {
  args: [FfiType.Handle, FfiType.Callback('continuation_callback')],
  ret: FfiType.Void,
  hasRustCallStatus: false,
}
// JS: nativeModule.uniffi_mylib_fn_poll_future(futureHandle, callbackFn)
```

**Example — VTable init function:**

```javascript
uniffi_mylib_fn_init_callback_vtable_mytrait: {
  args: [FfiType.Reference(FfiType.Struct('VTable_MyTrait'))],
  ret: FfiType.Void,
  hasRustCallStatus: true,
}
// JS: nativeModule.uniffi_mylib_fn_init_callback_vtable_mytrait(vtableObject, callStatus)
```

### `callbacks` — Map of callback function pointer signatures

Each key is a callback name (referenced by `FfiType.Callback(name)` in functions and structs). Each value is:

```javascript
{
  args: FfiType[],        // C argument types, excluding RustCallStatus
  ret: FfiType,           // C return type
  hasRustCallStatus: bool // true if C signature includes trailing &mut RustCallStatus
}
```

**Rules:**

- `args` must NOT include RustCallStatus
- The callback name must match what's used in `FfiType.Callback(name)` references
- `hasRustCallStatus: true` means the C function pointer type includes a trailing `&mut RustCallStatus` parameter that uniffi-runtime-napi handles internally

**Example — void continuation callback (async polling):**

```javascript
UniffiRustFutureContinuationCallback: {
  args: [FfiType.UInt64, FfiType.Int8],
  ret: FfiType.Void,
  hasRustCallStatus: false,
}
// C: void (*)(uint64_t, int8_t)
// JS callback receives: (data: bigint, pollResult: number)
```

**Example — VTable method with return value and status:**

```javascript
callback_mytrait_get_value: {
  args: [FfiType.Handle],
  ret: FfiType.Int32,
  hasRustCallStatus: true,
}
// C: int32_t (*)(uint64_t, RustCallStatus*)
// JS callback receives: (handle: bigint, callStatus: {code: number}) → number
```

**Example — VTable free function:**

```javascript
callback_mytrait_free: {
  args: [FfiType.Handle],
  ret: FfiType.Void,
  hasRustCallStatus: true,
}
// C: void (*)(uint64_t, RustCallStatus*)
// JS callback receives: (handle: bigint, callStatus: {code: number})
```

### `structs` — Map of C struct definitions (VTables)

Each key is a struct name (referenced by `FfiType.Struct(name)`). Each value is an array of field definitions **in C struct field order**:

```javascript
[
  { name: 'field_name', type: FfiType.* },
  ...
]
```

**Rules:**

- Fields must be in the same order as the C `#[repr(C)]` struct
- For VTables, all fields are typically `FfiType.Callback(name)`
- The callback names must have corresponding entries in `definitions.callbacks`

**Example — VTable for a foreign trait:**

```javascript
VTable_MyTrait: [
  { name: 'get_value', type: FfiType.Callback('callback_mytrait_get_value') },
  { name: 'set_value', type: FfiType.Callback('callback_mytrait_set_value') },
  { name: 'uniffi_free', type: FfiType.Callback('callback_mytrait_free') },
],
```

---

## Calling Convention at Call Sites

### Standard function call (hasRustCallStatus: true)

```javascript
const callStatus = { code: 0 };
const result = nativeModule.symbol_name(arg1, arg2, ..., callStatus);
// After call:
//   callStatus.code === 0  → success
//   callStatus.code !== 0  → error; callStatus.errorBuf is a Uint8Array with error details
```

### Standard function call (hasRustCallStatus: false)

```javascript
const result = nativeModule.symbol_name(arg1, arg2, ...);
```

### Passing a callback function argument

```javascript
const myCallback = (data, pollResult) => {
  // Called synchronously (if from main thread) or asynchronously (if from another thread)
};
nativeModule.uniffi_fn_with_callback(myCallback, otherArgs..., callStatus);
```

### Passing a VTable struct argument

```javascript
const vtable = {
  method_name: (handle, arg1, callStatus) => {
    callStatus.code = 0;
    return returnValue; // Must match the callback's ret type
  },
  uniffi_free: (handle, callStatus) => {
    callStatus.code = 0;
  },
};
nativeModule.uniffi_fn_init_vtable(vtable, callStatus);
```

### Callback argument types (inside callback JS functions)

When uniffi-runtime-napi calls a JS callback (either per-call or VTable), the arguments are marshaled from C:

```
C uint8_t   → JS number
C int8_t    → JS number
C uint16_t  → JS number
C int16_t   → JS number
C uint32_t  → JS number
C int32_t   → JS number
C uint64_t  → JS bigint
C int64_t   → JS bigint
C float     → JS number
C double    → JS number
```

If `hasRustCallStatus: true`, the last argument is a `{ code: number }` object. The callback should set `code` to `0` for success.

Callback return values are marshaled back to C using the same type mapping. For `FfiType.Void`, the return value is ignored.

---

## Complete Generated Module Template

```javascript
// Generated by uniffi-bindgen-node for crate "mylib"
import lib from "uniffi-runtime-napi/lib.js";
const { UniffiNativeModule, FfiType } = lib;

const mod = UniffiNativeModule.open(LIBRARY_PATH);

const nativeModule = mod.register({
  symbols: {
    rustbufferAlloc: "uniffi_mylib_rustbuffer_alloc",
    rustbufferFree: "uniffi_mylib_rustbuffer_free",
    rustbufferFromBytes: "uniffi_mylib_rustbuffer_from_bytes",
  },
  structs: {
    // One entry per VTable struct from ci.ffi_definitions()
    // where definition is FfiDefinition::Struct
  },
  callbacks: {
    // One entry per callback from ci.ffi_definitions()
    // where definition is FfiDefinition::CallbackFunction
  },
  functions: {
    // One entry per function from ci.ffi_definitions()
    // where definition is FfiDefinition::Function
  },
});

// Export the native module for use by the shared ubrn runtime
export { nativeModule };
```

---

## Gotchas and Edge Cases

1. **BigInt is required for 64-bit types.** Passing a regular `number` for `UInt64`, `Int64`, or `Handle` will throw. Always use `42n` or `BigInt(42)`.

2. **`hasRustCallStatus` affects the JS argument count.** If `true`, the last JS argument must be the status object. Don't include `RustCallStatus` in the `args` array.

3. **Callback names are cross-referenced.** A `FfiType.Callback('foo')` in a function arg or struct field must have a matching `callbacks.foo` definition.

4. **Struct field order matters.** Fields must match the C `#[repr(C)]` layout order exactly. The names are used to read properties from the JS object, but the order determines the memory layout.

5. **VTable init is called once.** The JS functions in a VTable are wrapped in persistent trampolines. Call the init function once at module startup, not per-instance.

6. **Cross-thread void callbacks are non-blocking.** If Rust calls a void callback from a background thread, the JS function runs asynchronously on the event loop. The Rust thread does not wait.

7. **RustBuffer copies data.** When passing a `Uint8Array` as a `RustBuffer` arg, the bytes are copied into Rust-managed memory via `rustbuffer_from_bytes`. When receiving a `RustBuffer` return, the bytes are copied into a new `Uint8Array` and the Rust buffer is freed. There is no shared memory.

8. **`symbols` is required in every `register()` call.** Each crate has its own RustBuffer symbol names. In a megazord, the library path is shared via `open()` but each `register()` provides its own symbols.

9. **Empty `structs`/`callbacks`/`functions` must still be present.** Use `{}` for empty maps. All keys are required in the definitions object.
