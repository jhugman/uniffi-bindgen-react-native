## Objects

Objects are structs with methods. They are passed-by-reference across the FFI.

```rust
#[derive(uniffi::Object)]
struct MyObject {
    my_property: u32,
}

#[uniffi::export]
impl MyObject {
    fn new(num: u32) -> Self {
        Self {
            my_property: num,
        }
    }

    #[uniffi::constructor(name = "create")]
    fn secondary_constructor(num: u32) -> Self {
        Self::new(num)
    }

    fn my_method(&self) -> String {
        format!("my property is {}", self.my_property)
    }
}
```

This produces Typescript with the following shape:

```typescript
interface MyObjectInterface {
    myMethod(): string;
}

class MyObject implements MyObjectInterace {
    public constructor(num: number) {
        // …
        // call into the `new` function.
    }

    public static create(num: number): MyObjectInterface {
        // … secondary constructor
        // call into `secondary_constructor` function.
    }

    myMethod(): string {
        // call into the `my_method` method.
    }
}
```

### Object interfaces

A supporting interface is constructed for each object, with the naming pattern: `${OBJECT_NAME}Interface`.

This is used for return values and arguments elsewhere in the generated code.

e.g. a Rust function, `my_object` that returns a `MyObject` is written in Typescript as:

```typescript
function myObject(): MyObjectInterface
```

This is to support mocking of Rust objects.

### Uniffi traits

Implementing the following traits in Rust causes the corresponding methods to be generated in Typescript:

Trait    | Typescript method | Return
-------- | ----------------- | -------
`Display`| `toString()`      | `string`
`Debug`  | `toDebugString()` | `string`
`Eq`     | `equals(other)`   | `boolean`
`Hash`   | `hashCode()`      | `bigint`
`Ord`    | `compareTo(other)`| `number` (i8: −1, 0, or 1)

### Garbage collection

When the object is [garbage collected](./gc.md#garbage-collected-objects-trigger-a-drop-call-into-rust), the Rust native peer is dropped.

If the Rust object needs to be explicitly dropped, use the `uniffiDestroy()` method.

This will cause the reference to the object to be freed. If this is the last reference to be freed, then the object itself is dropped.
