## Errors

In Javascript, errors are __thrown__ when an error condition is found.

When calling code which can throw, it is good practice to wrap that code in a `try`/`catch` block:

```typescript
try {
    const result = divide(42, 0); // throws
} catch (e: any) {
    // do something with the error.
}
```

In other languages, e.g. Java or Swift, the method that can throw __must__ declare it on the method signature. e.g.

In Java:
```java
float divide(float top, float bottom) throws MathException {}
```

while in Swift:
```swift
func divide(top: Float, bottom: Float) throws -> Float {}
```

In Rust, instead of throwing with try/catch, a method returns a `Result` enum.

```rust
#[derive(uniffi::Error)]
pub enum MathError {
    DivideByZero,
}

#[uniffi::export]
fn divide(top: f64, bottom: f64) -> Result<f64, MathError> {
    if bottom == 0.0 {
        Err(MathError::DivideByZero)
    } else {
        Ok(top / bottom)
    }
}
```

### Enums as Errors

Notice that `MathError` is not itself a special kind of object. In idiomatic Rust, this is usually an enum.

`uniffi-bindgen-react-native` converts these types of enums-as-errors in to JS Errors. Due to a limitation in `babel`, subclasses of `Error` do not evaluate `instanceof` as expected. For this reason, each variant has its own `instanceOf` static method.

```typescript
try {
    divide(x, y);
} catch (e: any) {
    if (MathError.instanceOf(e)) {
        e instanceof Error; // true
        e instanceof MathError; // false
    }

    if (MathError.DivideByZero.instanceOf(e)) {
        // handle divide by zero
    }
}
```

Enums-as-errors may also have properties. These are exactly the same as [other enums](./enums.md#enums-with-properties), except they subclass `Error`.

e.g.

```rust
enum MyRequestError {
    UrlParsing(String),
    Timeout { timeout: u32 },
    ConnectionLost,
}

#[uniffi::export]
fn make_request() -> Result<String, MyRequestError> {
    // dummy implmentation.
    return Err(MyRequestError::ConnectionLost);
}
```

In typescript:

```typescript
try {
    makeRequest();
} catch (e: any) {
    if (MyRequestError.instanceOf(e)) {
        switch (e.tag) {
            case MyRequestError_Tags.UrlParsing: {
                console.error(`Url is bad ${e.inner[0]}!`);
                break;
            }
            case MyRequestError_Tags.Timeout: {
                const { timeout } = e.inner;
                console.error(`Timeout after ${timeout} seconds!`);
                break;
            }
            case MyRequestError_Tags.ConnectionLost {
                console.error(`Connection lost!`);
                break;
            }
        }
    }
}

```

### Objects as Errors

As you may have gathered, in Rust errors can be anything including objects. In the rare occasions this may be useful:

```rust
#[derive(uniffi::Object)]
pub struct MyErrorObject {
    e: String,
}

#[uniffi::export]
impl MyErrorObject {
    fn message_from_rust(&self) -> String {
        self.e.clone()
    }
}

#[uniffi::export]
fn throw_object(message: String) -> Result<(), MyErrorObject> {
    Err(MyErrorObject { e: message })
}
```

This is used in Typescript, the error itself is __not__ the object.

```typescript
try {
    throwObject("a message")
} catch (e: any) {
    if (MyErrorObject.instanceOf(e)) {
        // NOPE
    }
    if (MyErrorObject.hasInner(e)) {
        const error = MyErrorObject.getInner(e);
        MyErrorObject.instanceOf(error); // true
        console.error(error.messageFromRust())
    }
}
```

## Rust `Error` is renamed as `Exception` in typescript

To avoid collisions with the ECMAScript standard `Error`, any Rust enums, objects and records called `Error` are renamed `Exception`.
