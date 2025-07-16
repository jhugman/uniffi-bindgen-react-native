The `uniffi.toml` file is a toml file used to customize [the generation of C++ and Typescript](https://mozilla.github.io/uniffi-rs/0.27/bindings.html).

As of time of writing we support `typescript.customTypes`, `kotlin.cdylib_name` and `kotlin.package_name`.

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
Keep in mind that this setting is only used when the Kotlin native bindings are generated.
If the `cdylib_name` is different from output library name, JNA won't be able to load the library and will fail silently.

```toml
[bindings.kotlin]
cdylib_name = "my_library_name"
```

### Kotlin package_name
The `package_name` is the package name that will be used in the generated Kotlin code. All the generated native classes will be placed in this package.

```admonish warning
`package_name` is used to determine which Kotlin classes should be ignored by proguard. If you use a different package name, you will need to setup proguard rules on your own.
```

```toml
[bindings.kotlin]
package_name = "com.example.myapp"
```
