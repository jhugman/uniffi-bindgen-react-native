# Callback interfaces

Callbacks and function literals are not directly supported by `uniffi-rs`.

However, [callback __interfaces__](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#the-uniffiexportcallback_interface-attribute) are, that is: instances of Typescript classes can be passed to Rust. The Typescript methods of those objects may then be called from Rust.

```rust
#[uniffi::export(callback_interface)]
pub trait MyLogger {
    fn is_enabled() -> bool;
    fn error(message: string);
    fn log(message: string);
}

#[uniffi::export]
fn greet_with_logger(who: String, logger: Box<dyn MyLogger>) {
    if logger.is_enabled() {
        logger.log(format!("Hello, {who}!"));
    }
}
```

In Typescript, this can be used:

```typescript
class ConsoleLogger implements MyLogger {
    isEnabled(): boolean {
        return true;
    }
    error(message: string) {
        console.error(messgae);
    }
    log(message: string) {
        console.log(messgae);
    }
}

greetWithLogger(new ConsoleLogger(), "World");
```

So-called [Foreign Traits](https://mozilla.github.io/uniffi-rs/latest/foreign_traits.html) can also be used. These are traits that can be implemented by either Rust or a foreign language: from the Typescript point of view, these are exactly the same as callback interfaces. They differ on the Rust side, using `Rc<>` instead of `Box<>`.


```rust
#[uniffi::export(with_foreign)]
pub trait MyLogger {
    fn error(message: string);
    fn log(message: string);
}

#[uniffi::export]
fn greet_with_logger(who: String, logger: Arc<dyn MyLogger>) {
    logger.log(format!("Hello, {who}!"));
}
```

These trait objects can be implemented by Rust or Typescript, and can be passed back and forth between the two sides of the FFI.

## Errors

Errors are propagated from Typescript to Rust:

```rust
#[derive(uniffi::Error)]
enum MyError {
    LoggingDisabled,
}

#[uniffi::export(callback_interface)]
pub trait MyLogger {
    fn is_enabled() -> bool;
    fn log(message: string) -> Result<(), MyError>;
}

#[uniffi::export]
fn greet_with_logger(who: String, logger: Box<dyn MyLogger>) -> Result<(), MyError> {
    logger.log(format!("Hello, {who}!"));
}
```

If an error is thrown in Typescript, it ends up in Rust:

```typescript
class ConsoleLogger implements MyLogger {
    isEnabled(): boolean {
        return false;
    }
    log(message: string) {
        if (!this.isEnabled()) {
            throw new MyError.LoggingDisabled();
        }
        console.log(message);
    }
}

try {
    greetWithLogger(new ConsoleLogger(), "World");
} catch (e: any) {
    if (MyError.instanceOf(e)) {
        switch (e.tag) {
            case MyError_Tags.LoggingDisabled: {
                // handle the logging disabled error.
                break;
            }
        }
    }
}
```
