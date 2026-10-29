# @ubjs/worker

A MessagePort channel implementation that drives a uniffi-bindgen-react-native player from another JavaScript context. The package provides both ends of the channel: a receiver for the worker hosting the player, and a sender for the page or other thread sending function calls.

Both the receiver and sender are built from the same `DEFINITIONS` table, ensuring protocol parity across contexts. The receiver also drives UniFFI's future poll loop: a `rust_future_poll_*` call whose continuation reports `MAYBE_READY` is re-polled in the worker on a microtask, and only `READY` is forwarded to the sender, so a wake and its next poll never straddle a message round trip. Typical usage: call `createReceiver(DEFINITIONS, player, port)` in the worker to listen for incoming calls, and `createSender(DEFINITIONS, port)` on the page to dispatch them.

This package targets the wasm2 player and is an early deliverable for running it in a worker. Code generation for the receiver and sender adapters is still in development.
