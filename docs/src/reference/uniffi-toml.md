The `uniffi.toml` file is a toml file used to customize [the generation of C++ and Typescript](https://mozilla.github.io/uniffi-rs/0.27/bindings.html).

To include the file when invoking `ubrn`, specify the path in the
[corresponding key of the config](../reference/config-yaml.md#bindings).

As of time of writing, `[bindings.typescript]` supports `logLevel`, `consoleImport`, `customTypes`, `strictObjectTypes`, `strictTypeChecking`, `strictByteArrays`, `forceAsync` and `jspi`; `[bindings.kotlin]` supports `cdylib_name` and `package_name`. Each is described below.

### Opting out of Interface generation

By default, `ubrn` generates [Object interfaces](../idioms/objects.md#object-interfaces) for all objects. To opt out of
this behavior, set `bindings.typescript.strictObjectTypes` to `true`.

```toml
[bindings.typescript]
strictObjectTypes = true
```

### Typescript strict byte arrays

By default, byte arrays in Rust (i.e. `Vec<u8>`) are translated into Typescript [`ArrayBuffer`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ArrayBuffer) instances. This is advantageous when it is desirable to use a byte array to transport a sequence of more complex types. 

However, not all projects use or want that functionality. To globally translate `Vec<u8>` into [`Uint8Array`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array) instead, set the `strictByteArrays` property of the typescript bindings to `true`.

```toml
[bindings.typescript]
strictByteArrays = true
```

### Typescript strict type checking

By default, generated Typescript files begin with `// @ts-nocheck`, so that `tsc` skips them and downstream projects are not troubled by type errors in code they did not write.

To have `tsc` check the generated files, set `strictTypeChecking` to `true`. This is chiefly useful when working on the generated code itself.

```toml
[bindings.typescript]
strictTypeChecking = true
```

### Forcing an async surface

`forceAsync` gives chosen types and functions an `async`/`Promise` surface in Typescript, without making them async in Rust. The call into Rust still runs synchronously, on the thread that made it.

```toml
[bindings.typescript]
forceAsync = true
```

The point is to let you migrate call sites ahead of time. Rust running off the main thread — whether as real Rust on a background thread, or as WASM on a worker — can only be called asynchronously, so every call site has to grow an `await`. `forceAsync` lets you make that change against a build whose behavior has not changed, and find out what it costs.

```admonish warning
**`forceAsync` moves no work off the main thread.** An awaited call blocks the Javascript thread for exactly as long as the synchronous one did. All that changes is the shape of the call site.
```

```admonish warning
The `await` at each call site is the shape you are migrating towards, and it is what any off-main-thread scheme will need. The rest of the generated surface is less settled: `asyncToString` is named that way only because the call underneath is still synchronous, and forced calls take no `AbortSignal` only because there is no `Future` to cancel. Expect both to be spelled differently once there is.
```

Set it to `true` to force everything in the crate, or to a list of names to force only those:

```toml
[bindings.typescript]
forceAsync = ["Widget", "makeFlatWidget"]
```

A name may be an object, a record, an enum, or a top-level function. Spelling doesn't matter: `make_flat_widget`, `makeFlatWidget` and `MakeFlatWidget` all name the same function.

#### What changes in the generated Typescript

Methods, constructors, top-level functions and trait methods of a forced type return a `Promise`. Errors arrive as a rejected promise, so `try`/`catch` needs an `await` to catch anything:

```typescript
// Without forceAsync.
const w = new Widget("yo");
const label = w.label();

// With forceAsync. The primary constructor is no longer a `constructor`, so it
// becomes a static factory — a Rust `new` is already named `create` in Typescript.
const w = await Widget.create("yo");
const label = await w.label();
```

The one method that cannot simply become async is `toString`: Javascript calls it to coerce an object into a string, and an async one hands back a `Promise`. So the `Display` trait is generated as `asyncToString` on a forced type, which no longer has a `toString` of its own. Coercing it — in a template literal, say — gets you the Javascript default of `[object Object]`.

```typescript
await w.asyncToString(); // "Widget(yo)", from the Display trait
await w.toDebugString(); // Debug, Eq, Hash and Ord keep their names
```

Forced calls take no `{ signal: AbortSignal }` option bag. There is no `Future` on the Rust side to cancel — see [task cancellation](../idioms/promises.md#task-cancellation).

#### Callback interfaces and trait interfaces

A callback interface, or a `[Trait, WithForeign]` interface, is implemented in Typescript and called from Rust. `forceAsync` cannot change that direction of travel, so naming one has no effect on its surface. It is only checked: if any of its methods is **synchronous**, generation fails and names the offending methods.

Rust calls into these types through a vtable, and each slot is sync or async according to the Rust method. On the way out, `forceAsync` only has to wrap a return value in a resolved promise; on the way in, it would have to hand a promise to a synchronous slot, which has no way to wait for it. Make the methods `async fn` in Rust, or leave the interface out of the list.

The [`force-async`](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures/force-async) and [`force-async-list`](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures/force-async-list) fixtures exercise both forms.

### Suspending web calls with JSPI

`jspi` enables experimental WebAssembly JavaScript Promise Integration on the
`web` (`wasm`) backend. A synchronous Rust function can call a
Promise-returning JavaScript import and suspend until it settles, while the
JavaScript event loop keeps running. Its generated TypeScript API returns a
`Promise` and checks the Rust error status only after settlement.

Set this in the exporting crate's `uniffi.toml`:

```toml
[bindings.typescript]
jspi = ["compute", "fetchBytes", "Processor"]
```

Names use the same case normalization as `forceAsync`: `fetchBytes` matches
`fetch_bytes`. A name selects either a top-level function or an object, record,
or enum type. A type selects its constructors, methods, and exported
trait helpers (`Display`, `Debug`, `Eq`, `Hash`, `Ord`). Select methods through
their owning type; individual method names and qualified names are not accepted.
`jspi = true` selects all eligible functions and types; `false` (the default) or
an empty list disables it.

Selected Rust `async fn` exports use a dedicated JSPI poll adapter, including
async methods and constructors on selected types. Their future creation remains
synchronous; the async body can suspend during polling. Unselected async APIs
retain their ordinary poll exports. Argument lifting during future creation and
custom conversion outside the poll must not suspend.
Callback interfaces and callback-capable trait interfaces are excluded by `true`
and rejected when named explicitly. Unknown or ambiguous names are errors too.
Destructors and infrastructure exports such as allocation, cloning, checksums,
and callback registration remain synchronous. Non-WASM backends reject an enabled
setting; see below for the `wasm2` setup and dispatcher details.

An object's selected primary constructor becomes an async static `create` factory;
use `await Processor.create(...)` instead of `new Processor(...)`. Alternate
constructors retain their names and return Promises. An alternate constructor named
`create` conflicts with this primary factory and must be renamed. Generated record
helpers such as `Payload.create({...})` and enum variant constructors stay
synchronous: they only construct JavaScript values. Exported Rust trait helpers
also return Promises; `Display` becomes `asyncToString()`.

Explicit destruction and garbage-collection cleanup stay synchronous, so Rust
`Drop` implementations must not reach suspending imports. In-flight method calls
hold their own cloned Rust handles. `uniffiDestroy()` releases the JavaScript
wrapper's handle immediately, prevents new calls on it, and allows already-started
calls to finish; the Rust object drops when the final handle is released.
For a block containing awaited calls, use `uniffiUseAsync` to release the wrapper
after success or rejection:

```typescript
const processor = await Processor.create(/* constructor arguments */);
// With interface-shaped returns, narrow to the concrete implementation first.
if (Processor.instanceOf(processor)) {
  const result = await processor.uniffiUseAsync(async (p) => {
    await p.prepare();
    return p.compute();
  });
}
```

`uniffiUse` remains a synchronous helper; it does not wait for a returned Promise.

Rust async APIs retain their optional `AbortSignal`. Cancellation is best effort:
if an active poll produces a value, that value can win the cancellation race.
Aborting cannot unwind a suspended JavaScript import; the call and its cleanup
wait for the import and poll to settle. A Promise that never settles can therefore
keep the future alive indefinitely. Poll continuation and Promise settlement are
separate signals; the runtime waits for both before repolling or completing.

An unexpected JSPI poll throw or rejection raises `UniffiInternalError.JspiPollError`
with the original `cause`. Treat it as fatal and discard the WASM instance; do not
continue calling into it or destroy its objects. The runtime removes the failed
call's continuation and abort listener but deliberately does not complete, cancel,
or free that future: a trap can bypass Rust destructors and leave its mutex locked.
This retains Rust resources until the instance is discarded. Other calls are not
automatically cancelled or disabled. Ordinary UniFFI errors and caught import
rejections use the normal completion and cleanup path.


JSPI is separate from `forceAsync`: it changes the WASM boundary to permit actual
suspension. A function selected by both settings is still a single Promise-returning
call. Unselected calls retain their existing behavior. JSPI does not discover
suspending imports by inspecting the Rust call graph; every entry point that
reaches one must be selected. Synchronous inbound callback frames must not suspend. Regular UniFFI async
foreign callbacks use their existing future protocol and do not require JSPI
suspension across the callback frame.

The exporting crate and generated wrapper crate need a JSPI-capable wasm-bindgen
version (tested with **0.2.128**), with the CLI at the exact same version. Update
older Cargo locks explicitly; adding this setting does not upgrade dependencies.
Use `UBRN_WASM_BINDGEN` to select a matching CLI. A JSPI-capable browser is required;
Node 24.14.0 needs both `--experimental-wasm-jspi` and
`--experimental-wasm-exnref`. Generated modules check JSPI support before importing
wasm-bindgen glue. Any additional optimizer must support the emitted exception
instructions (for wasm-opt, enable exceptions or skip optimization). See the
[wasm-bindgen JSPI guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/jspi.html).

Overlapping invocations have independent status wrappers, kept alive through
settlement and then consumed, including on rejection. Application state may still
need protection from reentrant calls. Import rejection can be caught in Rust and
returned as an ordinary UniFFI error. Do not rely on uncaught import rejection to
clean up arbitrary Rust resources across an `extern "C"` boundary; use `catch` on
fallible suspending imports and propagate a `Result`.

### Suspending wasm2 calls with JSPI

The `wasm2` player supports selected top-level functions and object/record/enum
types, including Rust async callables. Use the same boolean/name-list `jspi`
configuration and case normalization described above. `true` selects eligible
functions and types. Selecting a type includes its Rust constructors, methods
and exported trait helpers; callback-capable types are excluded and rejected
when named. Unsupported callback/reference ABI shapes are also excluded.

Selected primary object constructors become `await Type.create(...)`; alternate
constructors retain their names. JS-only record helpers and enum variants stay
synchronous. In-flight methods own cloned handles, so explicit destruction
prevents new calls while allowing existing calls to finish. Use
`uniffiUseAsync` for an awaited scope with cleanup on success or rejection.
Allocation, cloning, destruction and initialization stay synchronous; Rust
`Drop` must not suspend. `forceAsync` is independent.

The exporting cdylib must depend directly on a JSPI-capable wasm-bindgen
(tested with `0.2.128`) and use the matching CLI, as for `web`. Add this once in
the exporting crate's `lib.rs`:

```rust
#[cfg(target_arch = "wasm32")]
uniffi_runtime_wasm::export_jspi_entry!();
```

This macro emits an instrumented `__ubrn_jspi_enter` export in the consuming
crate. It does not add a wasm-bindgen dependency or change its version. Keep
the ordinary `uniffi-runtime-wasm` dependency/linkage and single-threaded UniFFI
configuration. `ubrn build wasm2` stages the growable table before wasm-bindgen;
do not run an incompatible optimizer over its JSPI output.

For example:

```toml
[bindings.typescript]
jspi = ["compute", "fetchBytes"]
```

Generated selected calls return Promises. The player emits small WASM thunks
for each signature; these call raw Rust exports through the instrumented entry
without an intervening JavaScript frame. Each invocation owns its call frame
until settlement. Selected calls lower buffers into JS-owned memory before
copying them into Rust allocations, and copy/free returned buffers before
resolving. This permits overlap, arbitrary completion order and memory growth
without handing detached views to generated converters. Unselected calls keep
their synchronous dispatch and buffer path.

Selected Rust async functions retain UniFFI's future protocol. Creation stays
synchronous and must not suspend while lifting arguments. Dedicated poll aliases
use the instrumented entry and a synchronous continuation callback; the runtime
waits for both the continuation and poll Promise before completing, repolling,
or freeing. Dedicated completion aliases copy/free owned results synchronously
before crossing an `await`. Original shared polls/completions remain unchanged
for unselected async APIs. Cancellation is best effort: it cannot unwind a
suspended import, cleanup waits for settlement, and a Ready result can win an
abort. Destructors and custom converters must not suspend.

Catch fallible suspending imports in Rust and propagate `Result`. An unexpected
entry throw/rejection raises `JspiCallError` with its cause, retains frames whose
Rust ownership is unknown, and disables further calls through that player's
registered APIs. Async calls wrap a failed poll in `JspiPollError`, retaining
its cause and skipping completion/free for that future. Discard the entire WASM
instance. Already-suspended imports
cannot be forcibly unwound, and raw export access bypasses this guard. Recoverable
UniFFI errors consume their error buffers and release their frames normally.
Destructors, synchronous inbound callbacks and custom converters must not suspend.
The runtime does not impose application-specific serialization.

### Logging the FFI

The generated Typescript code can optionally be created to generate logging.

```toml
[bindings.typescript]
logLevel = "debug"
consoleImport = "@/hermes"
```

`consoleImport` is an optional string which is the location of a module from which a `console` will be imported. This is useful in environments where `console` do not exist.

#### Log level

Possible values:

- `none`: The Uniffi generated Typescript produces no logging.
- `debug`: The generated Typescript records the call sites of `async` functions.
- `verbose`: As `debug` but also: all calls into Rust are logged to the console. This can be quite… verbose.

The recording of `async` call sites is also helpful for app development, so `process.env.NODE_ENV !== "production"` is checked at startup of runtime.

When `process.env.NODE_ENV === "production"`, async errors detected by Rust are reported but not with a helpful Typescript stack trace. Recording the call sites has a performance cost so is turned off for production.

### Typescript custom types

From [the uniffi-rs manual](https://mozilla.github.io/uniffi-rs/latest/udl/custom_types.html):

> Custom types allow you to extend the UniFFI type system to support types from your Rust crate or 3rd party libraries. This works by converting to and from some other UniFFI type to move data across the FFI.

This table customizes how a type called `MillisSinceEpoch` comes out of Rust.

We happen to know that it crosses the FFI as a Rust `i64`, which
converts to a JS `bigint`, but we can do better.

```toml
[bindings.typescript.customTypes.MillisSinceEpoch]
# Name of the type in the Typescript code.
typeName = "Date"
# Expression to lift from `bigint` to the higher-level representation `Date`.
lift = 'new Date(Number({}))'
# Expression to lower from `Date` to the low-level representation, `bigint`.
lower = "BigInt({}.getTime())"
```

This table customizes how a type called `Url` comes out of Rust.
We happen to know that it crosses the FFI as a `string`.

```toml
[bindings.typescript.customTypes.Url]
# We want to use our own Url class; because it's also called
# Url, we don't need to specify a typeName.
# Import the Url class from ../src/converters
imports = [ [ "Url", "../src/converters" ] ]
# Expressions to convert between strings and URLs.
# The `{}` is substituted for the value.
lift = "new Url({})"
lower = "{}.toString()"
```
We can provide zero or more imports which are slotted into a JS import statement. This allows us to import `type` and from modules in `node_modules`.

The next example is a bit contrived, but allows us to see how to customize a generated type that came from Rust.

The `EnumWrapper` is defined in Rust as:

```rust
pub struct EnumWrapper(MyEnum);
uniffi::custom_newtype!(EnumWrapper, MyEnum);
```

In the `uniffi.toml` file, we want to convert the wrapped `MyEnum` into a `string`. In this case, the `string` is the custom type, and we need to provide code to convert to and from the custom type.
```toml
[bindings.typescript.customTypes.EnumWrapper]
typeName = "string"
# An expression to get from the custom (a string), to the underlying enum.
lower = "{}.indexOf('A') >= 0 ? new MyEnum.A({}) : new MyEnum.B({})"
# An expression to get from the underlying enum to the custom string.
# It has to be an expression, so we use an immediately executing anonymous function.
lift = """((v: MyEnum) => {
    switch (v.tag) {
        case MyEnum_Tags.A:
            return v.inner[0];
        case MyEnum_Tags.B:
            return v.inner[0];
    }
})({})
"""
```

### Kotlin cdylib_name
The `cdylib_name` is the name of the library that will be loaded by JNA in the runtime. 
If the `cdylib_name` is different from output library name, JNA won't be able to load the library and will fail silently.

```toml
[bindings.kotlin]
cdylib_name = "my_library_name"
```

### Kotlin package_name
The `package_name` is the package name that will be used in the generated Kotlin code. All the generated native classes will be placed inside this package.

```toml
[bindings.kotlin]
package_name = "com.example.myapp.mycrate"
```

```admonish warning
If you are using proguard it is important to add the appropriate classes to proguard-rules.pro.
Otherwise, application in the release version may not work as it should.
```
