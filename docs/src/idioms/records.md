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
