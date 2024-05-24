## Records

Uniffi records are data objects whose fields are serialized and passed over the FFI. In UDL, they may be specified with the `dictionary` keyword:

```webidl
dictionary OptionneurDictionnaire {
  string mandatory_property;
  string defaulted_property = "Specified by UDL or Rust";
};
```

They are implemented as bare objects in Javascript, with a `type` declaration in Typescript.

```ts
type MyRecord = {
    mandatoryProperty: string,
    defaultedProperty: string,
};
```

Using this scheme however, loses the default values provided by the UDL, or Rust.

To correct this, `uniffi-bindgen-react-native` generates a companion factory object.

```ts
const MyRecordFactory = {
    create(fields: Missing<MyRecord>) { … }
    defaults(): Partial<MyRecord> { … }
};
```

The `Missing<MyRecord>` type above is a little bit handwavy, but it's defined as the union of non-defaulted fields, and the partial of the defaulted fields.

So, continuing with our example, the factory will be minimally happy with:

```ts
const myRecord = MyRecordFactory.create({
    mandatoryProperty: "Specified in Typescript"
});

assert(myRecord.mandatoryProperty === "Specified in Typescript");
assert(myRecord.defaultProperty === "Specified by UDL or Rust");
```
