For usage in Rust on how to use uniffi's proc-macros, see the `uniffi-rs` book for [Procedural Macros: Attributes and Derives](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html).

This section is about how the generated Typescript maps onto the Rust idioms available.

A useful way of organizing this is via the types that can be passed across the FFI.

### Simple scalar types

|   | Rust | Typescript |   |
| - | ---- | ---------- | - |
| Unsigned integers | `u8`, `u16`, `u32` | `number` | Positive numbers only |
| Signed integers   | `i8`, `i16`, `i32` | `number` | |
| Floating point    | `f32`, `f64` | `number` | |
| 64 bit integers   | `u64`, `i64` | `bigint` | [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt) |
| Strings           | `String` | `string` | UTF-8 encoded |

### Other simple types

|   | Rust | Typescript |   |
| - | ---- | ---------- | - |
| Byte array | `Vec<u8>` | `ArrayBuffer` | [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ArrayBuffer) |
| Timestamp | `std::time::SystemTime` | `Date` | aliased to `UniffiTimestamp` |
| Duration | `std::time::Duration` | `number` ms | aliased to `UniffiDuration`


### Structural types

|   | Rust | Typescript |   |
| - | ---- | ---------- | - |
| Optional | `Option<T>` | `T \| undefined` | |
| Sequences   | `Vec<T>` | `Array<T>` | Max length is 2**31 - 1|
| Maps    | `HashMap<K, V>` <br/> `BTreeMap<K, V>` | `Map<K, V>` | Max length is 2**31 - 1 |

### Enumerated types

|   | Rust | Typescript |   |
| - | ---- | ---------- | - |
| [Enums](./enums.md#enums-without-properties)    | `enum` | `enum` | [Flat enums](./enums.md#enums-without-properties)
| [Tagged Union Types](./enums.md#enums-with-properties) | `enum` | Tagged unions | [Enums with properties](./enums.md#enums-with-properties)
| [Error enums](./errors.md#enums-as-errors) | `enums` | `Error` | |

### Struct types

|   | Rust | Typescript | |
| - | ---------- | ---- | - |
| [Objects](./objects.md) | `struct Foo {}` | `class Foo` | class objects with methods
| [Records](./records.md) | `struct Bar {}` | `type Bar = {}` | objects without methods
| [Error objects](./errors.md#objects-as-errors) | `struct Baz {}` | `Error` | object is a property of the `Error` |
