# @ubjs/worker

A MessagePort channel implementation that drives a uniffi-bindgen-react-native player from another JavaScript context. The package provides both ends of the channel: a receiver for the worker hosting the player, and a sender for the page or other thread sending function calls.

Both the receiver and sender are built from the same `DEFINITIONS` table, ensuring protocol parity across contexts. An async function's entry on the player returns a Promise; the receiver awaits it and posts one `return`, and an `AbortSignal` the page passes is replayed as a one-way `cancel` message that aborts the receiver's own signal for that call. Typical usage: call `createReceiver(DEFINITIONS, player, port)` in the worker to listen for incoming calls, and `createSender(DEFINITIONS, port)` on the page to dispatch them.

Give a library one port, however many namespaces it has. A method call clones its object's handle on the port of the namespace that defines the object, without waiting for the reply, and nothing orders messages across two ports: with a port per namespace, a call to another namespace's function can overtake the clone, and Rust frees an object the page still holds. Bindings generated with `--async` do this for you. `index.port.ts` exports `uniffiInitAsync(port)`, which puts one sender over the library's merged table in front of every namespace, and `player-definitions.ts` exports that table for the receiver.

A port may carry a `closed` promise, for a far end that can die, such as a process. When it settles, the sender rejects its pending calls with `ChannelClosedError` and the receiver aborts its running ones. A sender created after it settled closes on the next microtask: a call made before then is still posted, and is rejected with `ChannelClosedError`. DOM and Node ports have no `closed`; a sender or receiver over one closes only when you call `close()`.

The receiver drives the wasm2 player (`@ubjs/wasm`) or the napi player (`@ubjs/node`). The page's side is generated; the player's side is yours: open the player and call `createReceiver`.
