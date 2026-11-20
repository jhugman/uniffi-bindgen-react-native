# @ubjs/worker

A MessagePort channel implementation that drives a uniffi-bindgen-react-native player from another JavaScript context. The package provides both ends of the channel: a receiver for the worker hosting the player, and a sender for the page or other thread sending function calls.

Both the receiver and sender are built from the same `DEFINITIONS` table, ensuring protocol parity across contexts. An async function's entry on the player returns a Promise; the receiver awaits it and posts one `return`, and an `AbortSignal` the page passes is replayed as a one-way `cancel` message that aborts the receiver's own signal for that call. Typical usage: call `createReceiver(DEFINITIONS, player, port)` in the worker to listen for incoming calls, and `createSender(DEFINITIONS, port)` on the page to dispatch them.

The receiver drives the wasm2 player (`@ubjs/wasm`) or the napi player (`@ubjs/node`). This package is an early deliverable for running a player off the main thread. Code generation for the receiver and sender adapters is still in development.
