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

You can see this in action in the [`futures-example` fixture](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures/futures-example/src), and the more complete [`futures` fixture](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures/futures).

## Passing Promises across the FFI

There is no support for passing a `Promise` or `Future` as an argument or error, in either direction.

## Task cancellation

Internally, `uniffi-rs` generates [a cancel function](https://docs.rs/uniffi/latest/uniffi/ffi/fn.rust_future_cancel.html) for each Future. On calling it, the `Future` is dropped.

This is accessible for every function that returns a `Promise` by passing an _optional_ `{ signal: AbortSignal }` option bag as the final argument.

Using the same Rust as above:

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

It can be used from Typescript, either without an `AbortSignal` as above, or with one passed as the final argument:

```typescript
const abortController = new AbortController();
setTimeout(() => abortController.abort(), 1000);
try {
    // Wait 1 hour for Hello, World!
    const message = await sayAfter(60 * 60 * 1000, "World", { signal: abortController.signal });
    console.log(message);
} catch (e: any) {
    e instanceof Error; // true
    e.name === "AbortError"; // true
}
```

This example calls into the `say_after` function, and the Rust would wait for 1 hour before returning. However, the `abortController` has its `abort` method called after 1 second.

```admonish warning
[Task cancellation for one language is… complicated](https://without.boats/blog/asynchronous-clean-up/). For FFIs, it is a small but important source of impedence mismatches between languages.

The [`uniffi-rs` docs](https://mozilla.github.io/uniffi-rs/latest/futures.html#cancelling-async-code) suggest that:

> You should build your cancellation in a separate, library specific channel; for example, exposing a `cancel()` method that sets a flag that the library checks periodically.

While `uniffi-rs` is recommending this, `uniffi-bindgen-react-native`— as a foreign-language binding to the uniffi-rs code— does so too.

However, while `uniffi-rs` exposes `rust_future_cancel` function, `uniffi-bindgen-react-native`— as a foreign-language binding to the uniffi-rs code— does so too.
```
