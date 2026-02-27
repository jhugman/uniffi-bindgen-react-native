## Records

[Uniffi records](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#the-uniffirecord-derive) are data objects whose fields are serialized and passed over the FFI i.e. pass by value.

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

## Methods

Records can have methods defined via `#[uniffi::export] impl`:

```rust
#[derive(uniffi::Record)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[uniffi::export]
impl Point {
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn scale(&self, factor: f64) -> Point {
        Point { x: self.x * factor, y: self.y * factor }
    }
}
```

Since records are plain data objects with no class instances, methods become **static-style functions on the companion factory object**, alongside `create` and `new`. The `self` parameter becomes the first argument:

```typescript
// Point has no defaulted fields, so create and new both accept all fields.
const p = Point.create({ x: 3.0, y: 4.0 });
const p2 = Point.new({ x: 3.0, y: 4.0 });   // synonym for create

const origin = Point.create({ x: 0.0, y: 0.0 });

Point.distanceTo(p, origin);   // 5.0
Point.scale(p, 2.0);           // { x: 6.0, y: 8.0 }
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
