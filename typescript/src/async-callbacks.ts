/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { CALL_ERROR, CALL_UNEXPECTED_ERROR } from "./rust-call.js";
import { type UniffiHandle, UniffiHandleMap } from "./handle-map.js";
import { type UniffiByteArray } from "./ffi-types.js";

// Some additional data we hold for each in-flight promise.
type PromiseHelper = {
  // The promise itself, so it doesn't get GCd by mistake.
  promise: Promise<any>;
  // The abort controller we will use to cancel, if necessary.
  abortController: AbortController;
  // A mutable object which gets set when the promise has succeeded or errored.
  // If uniffiForeignFutureFree gets called before settled is turned to true,
  // then we know that it is a call to cancel the task.
  settledHolder: {
    settled: boolean;
  };
};
const UNIFFI_FOREIGN_FUTURE_HANDLE_MAP = new UniffiHandleMap<PromiseHelper>();

// Some degenerate functions used for default arguments.
const notExpectedError = (err: any) => false;
function emptyLowerError<E>(e: E): UniffiByteArray {
  throw new Error("Unreachable");
}

// Callbacks passed into Rust.
type UniffiForeignFutureFree = (handle: bigint) => void;

export type UniffiForeignFuture = {
  handle: bigint;
  free: UniffiForeignFutureFree;
};

export function uniffiTraitInterfaceCallAsync<T>(
  makeCall: (signal: AbortSignal) => Promise<T>,
  handleSuccess: (value: T) => void,
  handleError: (
    callStatus: /*i8*/ number,
    errorBuffer: UniffiByteArray,
  ) => void,
  lowerString: (str: string) => UniffiByteArray,
): UniffiForeignFuture {
  return uniffiTraitInterfaceCallAsyncWithError(
    makeCall,
    handleSuccess,
    handleError,
    notExpectedError,
    emptyLowerError,
    lowerString,
  );
}

export function uniffiTraitInterfaceCallAsyncWithError<T, E>(
  makeCall: (signal: AbortSignal) => Promise<T>,
  handleSuccess: (value: T) => void,
  handleError: (
    callStatus: /*i8*/ number,
    errorBuffer: UniffiByteArray,
  ) => void,
  isErrorType: (error: any) => boolean,
  lowerError: (error: E) => UniffiByteArray,
  lowerString: (str: string) => UniffiByteArray,
): UniffiForeignFuture {
  const settledHolder: { settled: boolean } = { settled: false };
  const abortController = new AbortController();
  const promise = makeCall(abortController.signal)
    // Before doing anything else, we record that the promise has been settled.
    // Doing this after the `then` call means we only do this once all of that has finished,
    // which is way too late.
    .finally(() => (settledHolder.settled = true))
    .then(handleSuccess, (error: any) => {
      let message = error.message ? error.message : error.toString();
      if (isErrorType(error)) {
        try {
          handleError(CALL_ERROR, lowerError(error as E));
          return;
        } catch (e: any) {
          // Fall through to unexpected error handling below.
          message = `Error handling error "${e}", originally: "${message}"`;
        }
      }
      // This is the catch all:
      // 1. if there was an unexpected error causing a rejection
      // 2. if there was an unexpected error in the handleError function.
      handleError(CALL_UNEXPECTED_ERROR, lowerString(message));
    });

  const promiseHelper = { abortController, settledHolder, promise };
  const handle = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.insert(promiseHelper);
  return /* UniffiForeignFuture */ {
    handle,
    free: uniffiForeignFutureFree,
  };
}

function uniffiForeignFutureFree(handle: UniffiHandle) {
  const helper = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.remove(handle);
  // #JS_TASK_CANCELLATION
  //
  // This would be where the request from Rust to cancel a JS task would come out.
  // Check if the promise has been settled, and if not, cancel it.
  if (helper?.settledHolder.settled === false) {
    helper.abortController.abort();
  }
}

// For testing
export function uniffiForeignFutureHandleCount(): number {
  return UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.size;
}
