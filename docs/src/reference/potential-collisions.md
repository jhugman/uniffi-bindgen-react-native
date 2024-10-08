# Potential collisions

## Uniffi adding to your API

Both uniffi, and `uniffi-bindgen-react-native` tries to stay away from polluting the API with its own identifiers: one of the design goals of the library is to make your Rust library usable in the same way as an idiomatic library.

However, sometimes this is unavoidable.

The following are generated on your behalf, even if you did not specify them:

### Methods which may collide, because you can define methods in the same namespace

- `equals`: a method generated corresponding to [the `Eq` and `PartialEq` trait](../idioms/objects.md#uniffi-traits)
- `hashCode`: a method generated corresponding to [the `Hash` trait](../idioms/objects.md#uniffi-traits)
- `toDebugString`: a method generated corresponding to [the `Debug` trait](../idioms/objects.md#uniffi-traits)
- `toString`: a method generated corresponding to [the `Display` trait](../idioms/objects.md#uniffi-traits)
- `uniffiDestroy`: a method in every [object](../idioms/objects.md#garbage-collection) to aid garbage collection.

### Interfaces which may be declared, and collide with other types

- `${NAME}Interface`: may collide with another type.

e.g.

```rust
#[derive(uniffi::Object)]
struct Foo {}

#[derive(uniffi::Record)]
struct FooInterface {}
```

The naming of the tags enums for [tagged union enums](../idioms/enums.md#enums-with-properties) deliberately contains an underscore.

Class and enum class names go through a camel casing which makes this impossible to collide when naming a Rust enum something that collides with the generated Typescript.

e.g.

```rust
// This enum will have a tags enum called MyEnum_Tags
#[derive(uniffi::Enum)]
enum MyEnum {}

// This record will be called MyEnumTags in Typescript.
#[derive(uniffi::Record)]
struct MyEnum_Tags {}
```

## Non-collisions

These are methods or members that will not collide under any circumstances because they are defined at a level where user-generated members are not.

### Types versus Objects

Records and Enums are generated with both a Typescript `type` and a Javascript `object`, of the same name.

These `object`s of the same name will be referred to as factory objects or helper objects.

### Records, i.e. objects without methods

```typescript
type MyRecord = {
    myProperty: string;
};

const MyRecord = {
    defaults(): Partial<MyRecord>,
    create(missingMembers: Partial<MyRecord>): MyRecord,
    new: MyRecord.create,
};
```

`defaults`, `create` and `new` will never collide with `myProperty` because:
- `myProperty` is a member of an object __of type__ `MyRecord`. It is never a member of the object __called__ `MyRecord`.


### Enums with values

Enums define their shape types, with a utility object to hold the variant classes.

To a first approximation, the generated code is drawn:

```typescript
enum MyShape_Tags { Circle, Rectangle }
type MyShape =
    { tag: MyShape_Tags.Circle, inner: [number;]} |
    { tag: MyShape_Tags.Rectangle, inner: { length: number; width: number; }}

const MyShape = {
    Circle: class Circle { constructor(
        public tag: MyShape_Tags.Circle,
        public inner: [radius: number]) {}
        static instanceOf(obj: any): obj is Circle {}
        static new(…): Circle {}
    },
    Rectangle: class Rectangle { constructor(
        public tag: MyShape_Tags.Rectangle,
        public inner: { length: number; width: number; }) {}
        static instanceOf(obj: any): obj is Rectangle {}
        static new(…): Rectangle {}
    },
    instanceOf(obj: any): obj is MyShape {}
};
```

This allows us to construct variants with:

```typescript
const variant: MyShape = new MyShape.Circle(2.0);
MyShape.instanceOf(variant);
MyShape.Circle.instanceOf(variant);
```

The type `MyShape` is different to the const `MyShape`, and typescript can tell the difference based upon the context.

`tag`, `inner` and `instanceOf` do not collide with:
- variant names, which are all CamelCased.
- variant value names, which are isolated in the `inner` object.

### Static methods

- `create`: a static method in a Record Factory. User defined property, so will never be able
- `hasInner`: a static method added to object as Error classes.
- `getInner`: a static method added to object as Error classes.
- `instanceOf`: a static method added to Object, Enum, Enum variant and Error classes.
- `new`: a static method in record factory object
