# Lifting, lowering and serialization

This page is based upon the corresponding [`uniffi-rs` page](https://mozilla.github.io/uniffi-rs/latest/internals/lifting_and_lowering.html).

> UniFFI is able to transfer rich data types back-and-forth between the Rust code and the Typescript code via a process we refer to as "lowering" and "lifting".
>
> Recall that UniFFI interoperates between different languages by defining a C-style FFI layer which operates in terms of primitive data types and plain functions. To transfer data from one side of this layer to the other, the sending side "lowers" the data from a language-specific data type into one of the primitive types supported by the FFI-layer functions, and the receiving side "lifts" that primitive type into its own language-specific data type.
>
> Lifting and lowering simple types such as integers is done by directly casting the value to and from an appropriate type. For complex types such as optionals and records we currently implement lifting and lowering by serializing into a byte buffer, but this is an implementation detail that may change in future.

## Three layers
In many languages, there are mechanisms to talk to the C-style FFI layer directly. Javascript has no such facilities.

Instead, we perform serialization to an `ArrayBuffer` in Typescript, then pass it to C++ and then on to Rust.

This can be sketched as:
1. In Typescript, in `{namespace}.ts`: Lowering and serializing from higher level Typescript types to `ArrayBuffer`s, `number`s and `bigint`.
1. Javascript calls into C++, through `{namespace}-ffi.ts`
1. In C++, in `{namespace}.cpp`: lower the JSI `number`, `bigint` and `ArrayBuffer` further, into C equivalents, e.g. `uint32_t` and `uint8_t*`
1. Pass these C equivalents to Rust through a C style ABI.
1. Rust lifts the low level types, then calls into handwritten Rust.

In more detail, for most types:

1. do the [serialization (and deserialization)](https://mozilla.github.io/uniffi-rs/latest/internals/lifting_and_lowering.html#serialization-format) step in Typescript into an `ArrayBuffer` using `DataView` and `Uint8TypedArray`.
    - This is done with a series of `FfiConverter`s. These are generated for complex types, but many can be seen in [`ffi-converters.ts`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/typescript/src/ffi-converters.ts).
1. call into generated C++ with the `ArrayBuffer`.
    - This is represented by a `jsi::ArrayBuffer`.
    - The JS/C++ interface is defined on the Typescript side by the `{namespace}-ffi.ts` file; it is implemented by the `{namespace}.cpp` file.
1. [extract the `int32_t*` from the `jsi::ArrayBuffer`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/cpp/includes/ForeignBytes.h) and [copy into a `RustBuffer`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/react_native/gen_cpp/templates/RustBufferHelper.cpp), a [C-style struct shared by both Rust and C++](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/cpp/includes/RustBuffer.h).
1. call into Rust with the RustBuffer.

### Primitives
For more primitive types, the lifting and lowering is also done in two stages: for example, if Rust is expecting an `i32`, the Javascript `number` is passed into C++. The C++ then extracts the `int32_t` from the `jsi::Value::Number`.

### Strings
For Strings, we would want to use a [`TextEncoder`](https://developer.mozilla.org/en-US/docs/Web/API/TextEncoder). Unfortunately these aren't currently available for hermes; see hermes issues for [`TextEncoder`](https://github.com/facebook/hermes/issues/948) and [`TextDecoder`](https://github.com/facebook/hermes/issues/1403).

In this case, we use C++ again. When a string needs serializing to an `ArrayBuffer`, [the `FfiConverterString`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/react_native/gen_typescript/templates/StringHelper.ts):
1. calls passes the string from Typescript [to C++](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/crates/ubrn_bindgen/src/bindings/react_native/gen_cpp/templates/StringHelper.cpp), these are represented as `jsi::Value::String`.
1. In C++ [`UniffiString.h`](https://github.com/jhugman/uniffi-bindgen-react-native/blob/main/cpp/includes/UniffiString.h):
    1. get a C++ String using the `utf8()` method of `jsi::String`
    1. the copy the bytes into a `jsi::ArrayBuffer`.
1. Return the ArrayBuffer to Javascript so it can be:
    1. added to the serialization of a complex type OR
    1. passed to Rust as an `ArrayBuffer`, as above.
