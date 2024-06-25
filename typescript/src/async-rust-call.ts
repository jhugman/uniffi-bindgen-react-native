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

// TODO: this is hacked to make it compile. Make this work.
const uniffiContinuationHandleMap = new UniffiHandleMap<Promise<number>>();

type UniffiAsyncCallParams<F, T> = {
  rustFutureFunc: () => bigint;
  pollFunc: (
    rustFuture: bigint,
    cb: UniffiRustFutureContinuationCallback,
    handle: UniffiHandle,
  ) => void;
  completeFunc: (rustFuture: bigint, status: UniffiRustCallStatus) => F;
  freeFunc: (rustFuture: bigint) => void;
  liftFunc: (lower: F) => T;
  errorHandler?: UniffiErrorHandler;
};

export async function uniffiRustCallAsync<F, T>({
  rustFutureFunc,
  pollFunc,
  completeFunc,
  freeFunc,
  liftFunc,
  errorHandler,
}: UniffiAsyncCallParams<F, T>): Promise<T> {
  // Make sure to call uniffiEnsureInitialized() since future creation doesn't have a
  // UniffiRustCallStatus param, so doesn't use makeRustCall()
  // uniffiEnsureInitialized()
  const rustFuture = rustFutureFunc();

  try {
    let pollResult: number | undefined;
    do {
      // TODO this is to make it compile.
      pollResult = UNIFFI_RUST_FUTURE_POLL_READY;
      // pollResult = await withUnsafeContinuation(continuation => {
      //     pollFunc(
      //         rustFuture,
      //         uniffiFutureContinuationCallback,
      //         uniffiContinuationHandleMap.insert(continuation)
      //     )
      // })
    } while (pollResult != UNIFFI_RUST_FUTURE_POLL_READY);

    return liftFunc(
      makeRustCall((status) => completeFunc(rustFuture, status), errorHandler),
    );
  } finally {
    freeFunc(rustFuture);
  }
}

// Callback handlers for an async calls.  These are invoked by Rust when the future is ready.  They
// lift the return value or error and resume the suspended function.
const uniffiFutureContinuationCallback: UniffiRustFutureContinuationCallback = (
  handle: UniffiHandle,
  pollResult: number,
) => {
  const continuation = uniffiContinuationHandleMap.remove(handle);
  // TODO: make this work.
  // continuation.resume(pollResult)
};
