# The `wasm2` player

This page is for someone about to change the `wasm2` runtime. It builds from the ABI upwards: what UniFFI produces, what WebAssembly permits, and the one idea that connects them. For using the flavor, see [WebAssembly (`wasm2`) support](../reference/wasm2/overview.md).

## What UniFFI's ABI needs

UniFFI compiles a Rust crate into a `cdylib` of `extern "C"` functions, and every signature is drawn from a small vocabulary: the integer and float widths, plus a 64-bit `Handle` that indexes a Rust-side registry of objects; a `RustBuffer`, which is a `(capacity, len, dataPtr)` triple over bytes the Rust allocator owns, and through which records, strings, enums and options all travel; a `RustCallStatus` out-parameter, one status byte and a `RustBuffer` for the error; and function pointers, in vtables, for the calls Rust makes back into the foreign language.

Every UniFFI backend does the same three things with that vocabulary — lower the arguments, make the call, lift the result. See [Lifting, lowering and serialization](lifting-and-lowering.md) for the TypeScript side of it.

## What WebAssembly changes

Four differences drive nearly every decision in this runtime.

**There are no shared pointers.** Wasm has linear memory: one `ArrayBuffer` the module owns. Anything Rust reads must be bytes at an offset inside it, written by JavaScript. A `RustBuffer` argument is not a struct you pass — it is 24 bytes you write somewhere, plus the offset you wrote them at.

**Aggregates travel by pointer.** The wasm C ABI passes and returns structs through memory, so a function returning a `RustBuffer` compiles to one taking a hidden first argument: the address to write it to. The runtime has to know which functions those are.

**Rust reaches JavaScript only through the function table.** A Rust `fn` pointer on wasm32 is an index into `__indirect_function_table`, and `call_indirect` is the only instruction that can call one. A JavaScript closure is not a wasm function and cannot go in that table, so something must stand in for it.

**Memory can move.** `WebAssembly.Memory.grow` allocates a new backing buffer and detaches the old one, and any `Uint8Array` view over the old buffer becomes zero-length. Every view is short-lived by construction, or it is a bug.

Wasm32 is also single-threaded, which is why UniFFI's `wasm-unstable-single-threaded` feature is mandatory: without it, UniFFI demands `Send + Sync` on exported objects.

## The idea: a table, not a shim

The obvious way to bridge those two sections is to generate the bridging code, which is what the [`web` flavor](../guides/web/getting-started.md) does — for each FFI function it emits a Rust `#[wasm_bindgen]` wrapper into a generated shim crate, and builds it. The result is correct, and it costs a generated crate, a second `cargo` build, and a pile of JavaScript glue per project.

`wasm2` observes that those wrappers differ only in their *signatures*. Make the signature list data, and one runtime can drive every module. That runtime is the player.

So the bindgen emits two things: TypeScript that reads like your API, and a table describing every FFI export. It emits no glue.

```typescript
interface ModuleDefinitions {
    symbols: { rustbuffer_alloc: string; rustbuffer_free: string; rustbuffer_from_bytes: string };
    functions: Record<string, FunctionDef>;   // args, ret, hasRustCallStatus
    callbacks: Record<string, CallbackDef>;   // as above, plus outReturn
    structs: Record<string, FieldDesc[]>;     // vtables and result structs
}
```

The type vocabulary is the ABI's, made explicit in [`ffi-type.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/ffi-type.ts):

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

`Callback` and `Struct` name entries in the other two maps, so the table is self-contained. That is the whole interface between the bindgen and the runtime.

The same shape already served the [Node.js target](../reference/nodejs.md), which drives a native `cdylib` through libffi from an equivalent table. `wasm2` is the second consumer, and the two share the bindgen's IR:

```rust
pub enum AbiFlavor {
    Jsi,    // C++ turbo module, generated per crate
    Napi,   // player over libffi
    Wasm,   // generated wasm-bindgen shim crate
    Wasm2,  // player over WebAssembly
}
```

## Building: one cargo invocation

A UniFFI generator normally reads metadata out of a *native* library's symbol table, which is why the `web` flavor builds twice — once natively for metadata, once for wasm32 for the artifact.

There is no need. When rustc compiles a UniFFI crate to wasm, each `UNIFFI_META_*` symbol becomes an exported i32 global whose value is an address in linear memory, and the bytes at that address are the same self-describing blob a native build would hold. Reading them takes a wasm parser and three steps — collect the globals, find the exports whose names begin `UNIFFI_META`, resolve each address inside the active data segments — which is what [`wasm_metadata.rs`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/wasm_metadata.rs) does.

So the pipeline is:

1. `cargo build --lib --target wasm32-unknown-unknown`.
1. Read `UNIFFI_META_*` out of the resulting module, and generate the TypeScript from it.
1. Stage a *copy* of that module beside the TypeScript.

```admonish warning
Generate from cargo's output, and stage a copy. Staging rewrites the module, and one of its rewrites can remove the metadata exports. Keeping the two inputs distinct is what stops that from mattering.
```

### Staging

Three things happen to the copy, in [`ubrn_common::wasm`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_common/src/wasm.rs).

**wasm-bindgen runs, if the module needs it.** A crate whose dependencies reach `wasm-bindgen` links imports against a placeholder namespace only wasm-bindgen's rewriter can resolve. Staging asks the *import section* rather than scanning the file for the name — which also appears in the name section and in data segments — and if the answer is yes, runs the rewriter in-process against the bundler target. That leaves a `_bg.wasm`, renamed into place, and a `_bg.js` of glue. wasm-bindgen's own entry module is deleted: it instantiates the wasm for you, which is the player's job, and its filename would shadow the generated bindings under a bundler.

**The function table is exported, and made growable.** The player adds trampolines to `__indirect_function_table`, so the module must export it without an upper bound. `wasm-ld` will do that given `--export-table --growable-table`, but link arguments belong to the `cdylib`'s own link step, which a dependency cannot reach — every consumer would need `RUSTFLAGS`. Rewriting after the link works whatever built the module.

**Dead code is eliminated, for this project's fixtures only.** Stripping exports the player cannot reach lets a garbage-collection pass reclaim what they held, but the keep-list is a heuristic, and a user's `cdylib` may export symbols for a consumer the command line cannot see. So it runs on fixtures and never on your crate.

## Opening a module

[`UniffiNativeModule.open`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/module.ts) compiles and instantiates. Two details are load-bearing.

The import object is built from the module's *own* import section. Anything the caller did not supply, and the optional resolver could not, is filled with a stub — a function that throws when called, a zero global, a one-page memory. An unwired import then fails at the call that needs it, naming it, rather than failing at instantiation with a message about a name you have never seen, or worse, silently doing nothing.

`env.__ubrn_dispatch` is always supplied. It is the single import through which every JavaScript callback is reached, and it closes over a `UniffiNativeModule` that does not exist yet at instantiation time — so the closure checks, and throws if the module calls back during its own start section.

After instantiation the player checks for `__ubrn_alloc`, `__ubrn_free` and `memory`, carves a 64 KB scratch arena out of the first, and installs the panic hook.

## Registering: one function per export

`registerSync(definitions)` walks `definitions.functions` and builds one JavaScript function per FFI export. Building happens once; the per-call work is what is left.

For each export the player computes a *call layout* — one contiguous region of wasm memory holding, in order, the `RustCallStatus` if the function has one, the struct-return slot if the return travels by pointer, and one slot per argument that needs lowering into memory. That region is **reserved once, at registration**, from the [scratch arena](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/scratch.ts), and reused by every call. It is the single biggest reason calls are cheap, and the single biggest constraint on the design — see [Trade-offs](#trade-offs).

Then the player picks a dispatcher, in [`call.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/call.ts).

Where every argument and the return type are scalars or `RustBuffer`, it string-builds the dispatcher body with the offsets baked in as constants and compiles it with `new Function`. There is no loop over argument descriptors at call time and no closure indirection: the body reads like hand-written code, which is what lets an engine monomorphise it.

Everything else — callback arguments, vtable structs — runs a per-argument `prepare` closure. The player also takes this interpreted path when `new Function` is unavailable, which it probes once at registration, so a page under a strict Content-Security-Policy gets working bindings rather than an error.

```admonish warning
The two paths must agree exactly, down to details like coercing small integers with `| 0` and skipping the return read on the error path. They are written next to each other for that reason, and nothing but tests enforces it.
```

## A call, end to end

Take `scan_receipt(image: Vec<u8>) -> Result<Receipt, ScanError>`, whose FFI signature takes a `RustBuffer` and a `RustCallStatus`, and returns a `RustBuffer` through a hidden pointer.

1. The generated code calls `FfiConverterBytes.lower(image, rustbuffer_alloc)`. `rustbuffer_alloc(n)` returns a `Uint8Array` **aliasing wasm memory**, and the converter writes into it — no JavaScript-side buffer is built.
1. The dispatcher zeroes the status region.
1. It writes the 24-byte `RustBuffer` struct into the argument slot. Seeing that the view already aliases wasm memory, it forwards the view's byte offset as the data pointer: no second allocation, no copy.
1. It calls the export with `(sretAddress, argSlotAddress, statusAddress)`.
1. It reads the status byte. On success it reads the returned `(capacity, len, dataPtr)` and hands back a `Uint8Array` view over the payload. On failure it copies the error buffer out, frees it, and returns nothing — Rust has not written the return slot, and the caller is about to throw.
1. The generated code lifts the view into a `Receipt` inside a `try`, and frees it in the `finally`, so a throwing conversion still releases the memory.

Ownership, stated once: bytes going *into* Rust are Rust's, because the payload is allocated by the same global allocator Rust uses for `Vec<u8>`, so `Vec::from_raw_parts` reclaims it and the player must not. Bytes coming *out* are the caller's to free, through the view handed to it. Where Rust over-allocated — capacity above length — the true capacity rides on the view as a symbol-keyed property, because the allocator's `Layout` contract needs the size it was given.

## Callbacks: shapes and trampolines

Rust can only call a wasm function, so for every JavaScript closure the player emits one.

A *shape* is one wasm-level signature, say `(i64, i32) -> ()`. For each, [`trampoline.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/trampoline.ts) hand-encodes a wasm module — under a hundred bytes, built byte by byte in TypeScript — of exactly this form:

```wat
(module
  (type $trampoline (func (param ...) (result ...)))
  (type $dispatch   (func (param i32 ...) (result ...)))
  (import "env" "__ubrn_dispatch" (func $dispatch (type $dispatch)))
  (func $trampoline (type $trampoline)
    i32.const <shape_id>          ;; who am I
    local.get 0 ... local.get N   ;; forward everything
    call $dispatch)
  (export "trampoline" (func $trampoline)))
```

The trampoline prepends a shape id and forwards its arguments to a JavaScript function. That is all it does. Installing it means growing the module's function table by one and writing the export into the new slot, and the slot index is what Rust receives as the function pointer.

On the way back in, `__ubrn_dispatch(shapeId, ...args)` finds the closure and calls it, lifting each argument the mirror of the way the call path lowered them: a `RustBuffer` pointer becomes a `Uint8Array` and its wasm allocation is freed, Rust having handed ownership over; a nested function-table index becomes a callable that lowers *its* arguments and dispatches back through the table. Vtable methods return by out-parameter, so the dispatcher unwraps what the generated closure returned and writes the value and the status back through the pointers Rust supplied.

Each distinct closure-and-signature pair costs one shape and one table slot, never reclaimed. That would be alarming if closures were per-instance — but the generated code builds each callback interface's vtable once, at module scope, with instance identity travelling as a `Handle` argument, so an interface costs its method count plus two however many objects exist. Repeated installs of the same closure hit a cache and return the same slot.

```admonish note
A second dispatch path lives in [`callback.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/core/src/callback.ts): shapes shared by signature, with closures registered against them by handle. It is unit-tested and unused — today's generated code takes the per-closure path exclusively. Know that before you go looking for its callers.
```

## Async, and panics

Async needs no machinery of its own. UniFFI's poll-style ABI passes a continuation as a function pointer, so it is a `Callback` argument like any other: the shared runtime hands the player a JavaScript closure, the player installs it as a trampoline, and Rust calls it when the future can make progress. Each poll resolves one promise, the loop runs until the future reports ready, and a `complete` export then yields the value. Cancellation is a `cancel` export, wired to `AbortSignal`.

Panics cannot be seen from JavaScript by default — a wasm trap arrives with no message. So [the helper crate](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/runtimes/wasm/helper-crate/src/lib.rs) installs a Rust panic hook that formats the panic and calls a function pointer, and the player installs a JavaScript logger at that pointer through the same trampoline machinery.

The indirection through the function table is deliberate. An `env`-namespaced import would be simpler, but then every `cdylib` linking the helper crate would declare an import nothing can resolve, which breaks loaders that refuse unrecognised `env` modules. A table slot costs nothing and belongs to the module.

## Where the code lives

| Path | What |
| ---- | ---- |
| `runtimes/wasm/core/src/module.ts` | open, instantiate, register; owns the arena and the table |
| `runtimes/wasm/core/src/call.ts` | argument plans, call layout, both dispatchers |
| `runtimes/wasm/core/src/callback.ts` | shapes, dispatch routing, argument lift on the way in |
| `runtimes/wasm/core/src/trampoline.ts` | the hand-rolled wasm encoder |
| `runtimes/wasm/core/src/marshal.ts` | struct layouts, `RustBuffer` and `RustCallStatus` reads |
| `runtimes/wasm/core/src/{memory,scratch}.ts` | views over linear memory; the bump arena |
| `runtimes/wasm/{browser,node}/src` | how bytes are fetched; nothing else differs |
| `runtimes/wasm/helper-crate` | `uniffi-runtime-wasm`: alloc, free, panic hook |
| `crates/ubrn_bindgen/src/wasm_metadata.rs` | UniFFI metadata out of a `.wasm` |
| `crates/ubrn_bindgen/src/bindings/gen_typescript/ffi_module_player/` | the IR the signature table renders from |
| `crates/ubrn_common/src/wasm.rs` | staging: wasm-bindgen, table export, dead-code elimination |
| `crates/ubrn_cli/src/wasm2/` | `ubrn build wasm2`, `ubrn generate wasm2` |
| `crates/ubrn_fixture_testing/src/wasm2.rs` | the fixture harness |

The player package excludes itself from the cargo workspace and carries its own `package-lock.json`, because it publishes to npm as `@ubjs/wasm` and must build against a published `@ubjs/core` rather than against the checkout.

## Trade-offs

Where this design gives something up, it gives it up for a reason, and the reasons are worth knowing before you change one of them.

**Reserved scratch means no synchronous re-entry.** Every call to one export reuses one region, so a callback that synchronously calls back into the *same* export while the outer call is still on the stack overwrites that call's status byte, return slot and lowered arguments — silently. UniFFI's callback model does not generate that shape, and both dispatchers say so in a comment. The arena supports per-call push and pop for the day one does.

**The arena is 64 KB, and reservations are permanent.** Every registered function holds its layout for the module's life. A large megazord is the case that could exhaust it; the failure is loud and names the shortfall.

**Callback slots are never reclaimed.** Bounded in practice, as above, but a future codegen that installed per-instance closures would leak table slots.

**Two dispatchers must stay in step.** The compiled path is a meaningful win on the hot path and a real maintenance cost.

**Views alias wasm memory.** The alternative — copying every buffer to the JavaScript heap — would be safe and slower. The player takes the fast path and throws a specific error when the contract is broken, rather than reading garbage.

**The wasm-bindgen rewriter is version-locked.** It must match the schema of the `wasm-bindgen` crate the module was built against, so the workspace pins both. A bump is a coordinated change, not a dependency update.

**Metadata extraction reads module structure.** Exported i32 globals pointing into active data segments is how rustc and lld lay this out today, not a guarantee. A test extracts metadata from every fixture wasm to catch a change early.

**No threads, and no Hermes.** Both follow from the platform: the first shows up as a panic in Rust APIs that need a host thread, and the second means React Native proper stays with the JSI target.

## Testing

Four layers, each catching what the one below cannot.

**Unit tests** in `runtimes/wasm/core/tests`, run by `npm test`, drive the player against a hand-assembled minimal wasm module: enough exports to open, enough to call. They cover the arena, the encoder, struct layouts and both dispatchers without building a Rust crate.

**Fixture tests**, run by `cargo test -- wasm2::`, build a real fixture for wasm32, generate bindings from it, stage the module, and run the shared test script under Node.js. Eighteen of this project's twenty-one shared fixtures run this way, including callbacks, async, trait interfaces and external types. The `futures` fixture is excluded for a documented reason: its hand-rolled `TimerFuture` spawns a host thread.

**A build test**, `cargo test -p uniffi-bindgen-react-native --test wasm2_build`, runs `ubrn build wasm2` in a temporary project and asserts the files line up — that the name the bindings import is the name that was staged, that the generated wrapper stayed environment-neutral, that the staged module carries the table export. Nothing below this layer can catch a mismatch between two artifacts.

**Codegen tests**, in `crates/ubrn_bindgen/tests/wasm2_codegen.rs`, assert that the shared templates take the right branch for each flavor in both directions: that `wasm2` does not emit the Node.js loading path, and that Node.js still does.
