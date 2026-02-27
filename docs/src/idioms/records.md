## Records

[Uniffi records](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#the-uniffirecord-derive) are data objects whose fields are serialized and passed over the FFI i.e. pass by value. They do not have methods.

In UDL, they may be specified with the `dictionary` keyword:

```webidl
dictionary MyRecord {
  string mandatory_property;
  string defaulted_property = "Specified by UDL or Rust";
};
```

Alternatively, they are specified using a Rust proc-macro:

```rust
#[derive(uniffi::Record)]
struct MyRecord {
    mandatory_property: String,
    #[uniffi(default = "Specified by UDL or Rust")]
    defaulted_property: String,
}
```

They are implemented as bare objects in Javascript, with a `type` declaration in Typescript.

```ts
type MyRecord = {
    mandatoryProperty: string,
    defaultedProperty: string,
};
```

Using this scheme alone however, Typescript cannot represent the default values provided by the UDL, or Rust.

To correct this, `uniffi-bindgen-react-native` generates a companion factory object.

```ts
const MyRecord = {
    create(fields: Missing<MyRecord>) { … },
    defaults(): Partial<MyRecord> { … },
    new: create // a synonym for `create`.
};
```

The `Missing<MyRecord>` type above is a little bit hand-wavy, but it's defined as the union of non-defaulted fields, and the partial of the defaulted fields.

So, continuing with our example, the factory will be minimally happy with:

```ts
const myRecord = MyRecord.create({
    mandatoryProperty: "Specified in Typescript"
});

assert(myRecord.mandatoryProperty === "Specified in Typescript");
assert(myRecord.defaultProperty === "Specified by UDL or Rust");
```

## Uniffi traits

Implementing the following traits in Rust causes the corresponding methods to be generated in Typescript:

Trait    | Typescript method      | Return
-------- | --------------------- | -------
`Display`| `toString()`          | `string`
`Debug`  | `toDebugString()`     | `string`
`Eq`     | `equals(value, other)`| `boolean`
`Hash`   | `hashCode()`          | `bigint`
`Ord`    | `compareTo(value, other)`| `number` (i8: −1, 0, or 1)

Note: since records have no class instance, `equals` and `compareTo` take the record value as their first argument: `TraitRecord.equals(a, b)` rather than `a.equals(b)`.

These are declared on the record using the `#[uniffi::export(...)]` attribute:

```rust
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Record)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub struct TraitRecord {
    pub name: String,
    pub value: i32,
}

impl std::fmt::Display for TraitRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TraitRecord({}, {})", self.name, self.value)
    }
}
```

Unlike objects and enums (where these methods are instance methods), records are plain data objects with no class instances. Because of this, the trait methods become **static-style methods on the companion factory object**:

```typescript
const r = { name: "hello", value: 42 };

TraitRecord.toString(r);          // "TraitRecord(hello, 42)"
TraitRecord.toDebugString(r);     // 'TraitRecord { name: "hello", value: 42 }'

const a = { name: "x", value: 1 };
const b = { name: "x", value: 1 };
const c = { name: "x", value: 2 };

TraitRecord.equals(a, b);         // true
TraitRecord.equals(a, c);         // false

TraitRecord.hashCode(r);          // bigint

TraitRecord.compareTo(a, c);      // negative (1 sorts before 2)
TraitRecord.compareTo(c, a);      // positive
```
