# Promise / Futures

`uniffi-bindgen-react-native` provides support of `Future`s/`async fn`. These are mapped to Javascript `Promise`s. More information can be found in [the uniffi book](https://mozilla.github.io/uniffi-rs/latest/futures.html).

This example is taken from the above link:

```rust
use std::time::Duration;
use async_std::future::{timeout, pending};

/// Async function that says something after a certain time.
#[uniffi::export]
pub async fn say_after(ms: u64, who: String) -> String {
    let never = pending::<()>();
    timeout(Duration::from_millis(ms), never).await.unwrap_err();
    format!("Hello, {who}!")
}
```

It can be called from Typescript:

```typescript
// Wait 1 second for Hello, World!
const message = await sayAfter(1000n, "World");
```

## Calling async typescript

The following code calls an async function implemented by Typescript.
```rust
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait Greeter {
    async fn create_greeting(&self, delay_ms: i32, who: String) -> String;
}

async fn greet_with(
            greeter: &Greeter,
            delay_ms: i32,
            who: String
) -> String {
    greeter.create_greeting(delay_ms, who).await
}
```

The typescript:

```typescript
class TsGreeter implements Greeter {
    async createGreeting(delayMs: number, who: string) {
        await new Promise((resolve) => setTimeout(resolve, delayMs));
        return `Hello, ${who} from Typescript!`;
    }
}

const message = greetWith(new TsGreeter(), 1000, "World");
```

There is no support for passing a `Promise` or `Future` as an argument or error, in either direction.
