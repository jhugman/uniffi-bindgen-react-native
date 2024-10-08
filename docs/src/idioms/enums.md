<!-- toc -->

## Enums without properties

Enums with variants that have no properties are said to be "flat enums".

```rust
#[derive(uniffi::Enum)]
enum MyAnimal {
    Cat,
    Dog,
}
```

These are represented by a similar enum in Typescript:

```typescript
enum MyAnimal {
    Cat,
    Dog,
}
```

Constructing these in Typescript is done as usual:

```typescript
const dog = MyAnimal.Dog;
const cat = MyAnimal.Cat;
```

## Enums with properties

Rust enums variants optionally have properties. These may be name or unnamed.

```rust
#[derive(uniffi::Enum)]
enum MyShape {
  Point,
  Circle(f64),
  Rectangle { length: f64, width: f64, colour: String },
}
```

These may be constructed like so:

```rust
let p = MyShape::Point;
let c = MyShape::Circle(2.0);
let r = MyShape::Rectangle { length: 1.0, width: 1.0, colour: "blue".to_string(), };
```

These can be used in pattern matching, for example:

```rust
fn area(shape: MyShape) -> f64 {
  match shape {
    MyShape::Point => 0.0,
    MyShape::Circle(radius) => PI * radius * radius,
    MyShape::Rectangle { length, width, .. } => length * width,
  }
}
```

Such enums are in all sorts of places in Rust: `Option`, [`Result` and Errors](./errors.md) all use this language feature.

In Typescript, we don't have enums with properties, but we can simulate them:

```ts
enum MyShape_Tags { Point, Circle, Rectangle };
type MyShape =
    { tag: MyShape_Tags.Point } |
    { tag: MyShape_Tags.Circle, inner: [number] } |
    { tag: MyShape_Tags.Rectangle, inner: { length: number, width: number, colour: string }};
```

In order to make them easier to construct, a helper object containing sealed classes implementing the tag/inner:

```ts
const point = new MyShape.Point();
const circle = new MyShape.Circle(2.0);
const rectangle = new MyShape.Circle({ length: 1.0, width: 1.0, colour: "blue" });
```

These are arranged so that the Typescript compiler can derive the types when you match on the `tag`:

```ts
function area(shape: MyShape): number {
    switch (shape.tag) {
        case MyShape_Tags.Point:
            return 0.0;
        case MyShape_Tags.Circle: {
            const [radius] = shape.inner;
            return Math.PI * radius ** 2;
        }
        case MyShape_Tags.Rectangle: {
            const [length, width] = shape.inner;
            return length * width;
        }
    }
}
```

### `instanceOf` methods

Both the `enum` and each variant have `instanceOf` methods. These may be useful when you don't need to `match`/`switch` on all variants in the Enum.

```typescript
function colour(shape: MyShape): string | undefined {
    if (MyShape.Rectangle.instanceOf(shape)) {
        // We know what the type inner is.
        return shape.inner.colour;
    }
    return undefined;
}
```

```admonish tip
Adding one or more properties to one or more variants moves these flat enums to being "non-flat", as above.

To help switch between the two, the classes representing the variants have a static method `new`. For example, adding a property to the MyAnimal enum above:
```

```rust
#[derive(uniffi::Enum)]
enum MyAnimal {
    Cat,
    Dog(String),
}
```

This would mean changing the typescript construction to:

```typescript
const dog = new MyAnimal.Dog("Fido");
const cat = new MyAnimal.Cat();
```

The variants each have a static `new` method to have a smaller diff:

```typescript
const dog = MyAnimal.Dog.new("Fido");
const cat = MyAnimal.Cat.new();
```

## Enums with explicit discriminants

Both [Rust](https://doc.rust-lang.org/reference/items/enumerations.html#discriminants) and Typescript allow you to specify discriminants to enum variants. As [in other bindings for uniffi-rs](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#variant-discriminants), this is suppoorted by `uniffi-bindgen-react-native`. For example,

```rust
#[derive(uniffi::Enum)]
pub enum MyEnum {
    Foo = 3,
    Bar = 4,
}
```

will cause this Typescript to be generated:

```typescript
enum MyEnum {
    Foo = 3,
    Bar = 4,
}
```
