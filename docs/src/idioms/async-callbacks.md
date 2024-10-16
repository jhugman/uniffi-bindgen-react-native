# Async Callback interfaces

[Callback interfaces and foreign traits](./callback-interfaces.md) can expose methods which are asynchronous. A toy example here:

```rust
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
trait MyFetcher {
    async get(url: String) -> String;
}

fetch_with_fetcher(url: String, fetcher: Arc<dyn MyFetcher>) -> String {
    fetcher.fetch(url).await
}
```

Used from Typescript:

```typescript
class TsFetcher implements MyFetcher {
    async get(url: string): Promise<string> {
        return await fetch(url).text()
    }
}

fetchWithFetcher("https://example.com", new TsFetcher());
```

You can see this in action in the [`futures` fixture](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/fixtures/futures).

## Task cancellation

When the Rust Future is completed, it is dropped, and Typescript is informed. If the Future is dropped before it has completed, it has been cancelled. `uniffi-bindgen-react-native` can use this information to call the async callback to cancel, using [the standard `AbortController` and `AbortSignal`](https://developer.mozilla.org/en-US/docs/Web/API/AbortController) machinery.

`uniffi-bindgen-react-native` generates an optional argument for each async callback method, which is an options bag containing an `AbortSignal`.

It is up to the implementer of each method whether they want to use it or not.

Using exactly the same `MyFetcher` trait from above, this example passes the signal straight to [the `fetch` API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch#canceling_a_request).

```typescript
class TsFetcher implements MyFetcher {
    async get(url: string, asyncOptions?: { signal: AbortSignal }): Promise<string> {
        return await fetch(url, asyncOptions).text()
    }
}

fetchWithFetcher("https://example.com", new TsFetcher());
```
