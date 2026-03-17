# A test against the `wasm-unstable-single-threaded` feature

This fixture tests several situations where a compile time error would ordinarily occur, but enabling the `wasm-unstable-single-threaded` feature of `uniffi` allows this.

It also demonstrates several patterns to use when building for WASM32.

In this fixture, the API between JSI and WASM is identical. If you want to vary APIs between build targets, you should use a `feature` instead.
