The `uniffi.toml` file is a toml file used to customize [the generation of C++ and Typescript](https://mozilla.github.io/uniffi-rs/0.27/bindings.html).

As of time of writing, only `typescript` bindings generation exposes any options for customization, and only for `customTypes`.

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
