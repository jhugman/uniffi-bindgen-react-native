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

## Implementing traits from external crates

A trait defined in a dependency crate — not your own — can also be implemented in TypeScript, with no extra configuration. This works for both proc-macro style (`#[uniffi::export(with_foreign)]`) and UDL style (`[Trait, WithForeign]`) traits.

Suppose a dependency crate (`uniffi-one`) exports a foreign trait:

```rust
// in the `uniffi-one` crate
#[uniffi::export(with_foreign)]
pub trait UniffiOneTrait: Send + Sync {
    fn hello(&self) -> String;
}
```

Or equivalently via UDL:

```udl
// in uniffi-one.udl
[Trait, WithForeign]
interface UniffiOneUDLTrait {
    string hello();
};
```

Another crate (or your own app's Rust layer) can then accept the trait as a parameter:

```rust
// in a second crate that depends on `uniffi-one`
#[uniffi::export]
fn call_trait_impl(t: Arc<dyn UniffiOneTrait>) -> String {
    t.hello()
}
```

On the TypeScript side, import the interface from the external crate's generated bindings and implement it as usual:

```typescript
import { UniffiOneTrait } from "../generated/uniffi_one_ns";
import { callTraitImpl } from "../generated/imported_types_sublib";

const tsImpl: UniffiOneTrait = {
    hello(): string {
        return "hello from TypeScript";
    },
};

const result = callTraitImpl(tsImpl);
```

The generated `UniffiOneTrait` interface comes from `uniffi-one`'s bindings. Your crate's bindings expose `callTraitImpl`, which accepts any object satisfying that interface — whether it was created in Rust or TypeScript. No special annotation or glue code is needed.

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
