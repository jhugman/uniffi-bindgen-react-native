# @ubjs/message-channel

A MessagePort channel implementation that drives a uniffi-bindgen-react-native player from another JavaScript context. The package provides both ends of the channel: a receiver for the worker hosting the player, and a sender for the page or other thread sending function calls.

Both the receiver and sender are built from the same `DEFINITIONS` table, ensuring protocol parity across contexts. Typical usage: call `createReceiver(DEFINITIONS, player, port)` in the worker to listen for incoming calls, and `createSender(DEFINITIONS, port)` on the page to dispatch them.

This package targets the wasm2 player and is an early deliverable for message-channel transport. Code generation for the receiver and sender adapters is still in development.
