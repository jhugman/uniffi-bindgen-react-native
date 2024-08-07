/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiHandleMap, type UniffiHandle } from "./handle-map";
import {
  type UniffiErrorHandler,
  type UniffiRustCallStatus,
  makeRustCall,
} from "./rust-call";

const UNIFFI_RUST_FUTURE_POLL_READY = 0;
const UNIFFI_RUST_FUTURE_POLL_MAYBE_READY = 1;

// The UniffiRustFutureContinuationCallback is generated in the {{ namespace }}-ffi.ts file,
// when iterating over `ci.ffi_definitions()`.
//
// In binding generators for other languages, we would use that; however, in this binding, we've
// separated out the runtime from the generated files.
//
// We check if this is the same as the generated type in the {{ namespace }}-ffi.ts file.
// If a compile time error happens in that file, then uniffi-core has changed the way
// it is calling callbacks and this file will need to be changed.
export type UniffiRustFutureContinuationCallback = (
  handle: UniffiHandle,
  pollResult: number,
) => void;

type PollFunc = (
  rustFuture: bigint,
  cb: UniffiRustFutureContinuationCallback,
  handle: UniffiHandle,
) => void;

/**
 * This method calls an asynchronous method on the Rust side.
 *
 * It manages the impedence mismatch between JS promises and Rust futures.
 *
 * @param rustFutureFunc calls the Rust client side code. Uniffi machinery gives back
 *  a handle to the Rust future.
 * @param pollFunc is then called periodically. This sends a JS callback, and the RustFuture handle
 *  to Rust. In practice, this poll is implemented as a Promise, which the callback resolves.
 * @param cancelFunc is currently unexposed to client code.
 * @param completeFunc once the Rust future polls as complete, the completeFunc is called to get
 *  the result and any errors that were encountered.
 * @param freeFunc is finally called with the Rust future handle to drop the now complete Rust
 *  future.
 */
export async function uniffiRustCallAsync<F, T>(
  rustFutureFunc: () => bigint,
  pollFunc: PollFunc,
  cancelFunc: (rustFuture: bigint) => void,
  completeFunc: (rustFuture: bigint, status: UniffiRustCallStatus) => F,
  freeFunc: (rustFuture: bigint) => void,
  liftFunc: (lower: F) => T,
  liftString: (arrayBuffer: ArrayBuffer) => string,
  errorHandler?: UniffiErrorHandler,
): Promise<T> {
  // This actually calls into the client rust method.
  const rustFuture = rustFutureFunc();

  // We now poll the Rust future until it's ready.
  // The poll, complete and free methods are specialized by the FFIType of the return value.
  try {
    let pollResult: number | undefined;
    do {
      // Calling pollFunc with a callback that resolves the promise that pollRust
      // returns: pollRust makes the promise, uniffiFutureContinuationCallback resolves it.
      pollResult = await pollRust((handle) => {
        pollFunc(rustFuture, uniffiFutureContinuationCallback, handle);
      });
    } while (pollResult != UNIFFI_RUST_FUTURE_POLL_READY);

    // Now it's ready, all we need to do is pick up the result (and error).
    return liftFunc(
      makeRustCall(
        (status) => completeFunc(rustFuture, status),
        liftString,
        errorHandler,
      ),
    );

    // #RUST_TASK_CANCELLATION: the unused `cancelFunc` function should be exposed
    // to client code in order for clients to be able to cancel the running Rust task.
  } finally {
    freeFunc(rustFuture);
  }
}

// The resolver handle map contains the resolvers from each of the pollRust promises.
type PromiseResolver<T> = (value: T) => void;
const UNIFFI_RUST_FUTURE_RESOLVER_MAP = new UniffiHandleMap<
  PromiseResolver<number>
>();

// pollRust makes a new promise, stores the resolver in the resolver map,
// then calls the pollFunc with the handle.
function pollRust(pollFunc: (handle: UniffiHandle) => void): Promise<number> {
  return new Promise<number>((resolve) => {
    const handle = UNIFFI_RUST_FUTURE_RESOLVER_MAP.insert(resolve);
    pollFunc(handle);
  });
}

// Rust calls this callback, which resolves the promise returned by pollRust.
const uniffiFutureContinuationCallback: UniffiRustFutureContinuationCallback = (
  handle: UniffiHandle,
  pollResult: number,
) => {
  const resolve = UNIFFI_RUST_FUTURE_RESOLVER_MAP.remove(handle);
  resolve(pollResult);
};

// For testing only.
export function uniffiRustFutureHandleCount(): number {
  return UNIFFI_RUST_FUTURE_RESOLVER_MAP.size;
}
