# Threading

Javascript—and by extension, Typescript—is a single-threaded language.

Uniffi purposely does not get involved in threads, but does take its lead from the host language.

## React Native

The Rust integration for React Native provides the potential for multi-threaded Rust.

Dispatching to a background thread is an exercise for the developer on the Rust side of the FFI.

But what happens for background threads calling into Javascript? In such cases, because any client callbacks:
- may return something
- may throw an error that is declared and expected
- may throw an unexpected error

the background thread on the Rust side **must** block, waiting for Javascript callback to complete.

This might lead to a non-obvious deadlock position, if a `Mutex` is held while the callback is being called, and then is contended by the foreground call into the Rust, e.g. by a update or repaint.

This can be mitigated by one of:
- ensuring that a Mutex is released before the callback is called
- making the callback async and scheduling the repaint on the next tick.

## WASM

WASM is currently a single-threaded environment. At the time of writing, the migration path to a multi-threaded virtual machine is unclear.

`uniffi-rs` provides a [`wasm-unstable-single-threaded` feature][uniffi-rs-wasm]. This should be enabled in the target crate.

[uniffi-rs-wasm]: https://mozilla.github.io/uniffi-rs/latest/wasm/configuration.html

Additionally, the following may be helpful to adapt your Rust for running with uniffi and WASM.

### Async trait:

You may have code using the `async_trait` crate:

```rust
#[uniffi::export]
#[async_trait::async_trait]
pub trait MyRustTrait {
    async fn do_something_on_the_background(&self, ms: u16, who: String) -> String;
}
```

If `do_something_on_the_background` in turn awaits something in the browser, e.g. a fetch or a timer, these things are not `Send`,
in which case, you should re-write the `#[async_trait::async_trait]` to not be `Send` for `wasm32` targets.

```rust
#[uniffi::export]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait MyRustTrait {
    async fn do_something_on_the_background(&self, ms: u16, who: String) -> String;
}
```

Alternatively, you can forgo `async_trait` altogether, in favor of removing the `clippy` lint about async functions in traits.

```rust
#[uniffi::export]
#[allow(async_fn_in_trait)]
pub trait MyRustTrait {
    async fn do_something_on_the_background(&self, ms: u16, who: String) -> String;
}
```
