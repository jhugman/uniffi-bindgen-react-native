/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { CALL_ERROR, CALL_UNEXPECTED_ERROR } from "./rust-call";
import {
  type UniffiHandle,
  UniffiHandleMap,
  defaultUniffiHandle,
} from "./handle-map";

const UNIFFI_FOREIGN_FUTURE_HANDLE_MAP = new UniffiHandleMap<Promise<any>>();

const notExpectedError = (err: any) => false;
function emptyLowerError<E>(e: E): ArrayBuffer {
  throw new Error("Unreachable");
}

export type UniffiForeignFutureFree = (handle: bigint) => void;

export type UniffiForeignFuture = {
  handle: bigint;
  free: UniffiForeignFutureFree;
};

export function uniffiTraitInterfaceCallAsync<T>(
  makeCall: () => Promise<T>,
  handleSuccess: (value: T) => void,
  handleError: (callStatus: /*i8*/ number, errorBuffer: ArrayBuffer) => void,
  lowerString: (str: string) => ArrayBuffer,
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
  makeCall: () => Promise<T>,
  handleSuccess: (value: T) => void,
  handleError: (callStatus: /*i8*/ number, errorBuffer: ArrayBuffer) => void,
  isErrorType: (error: any) => boolean,
  lowerError: (error: E) => ArrayBuffer,
  lowerString: (str: string) => ArrayBuffer,
): UniffiForeignFuture {
  const promise = makeCall().then(handleSuccess, (error: any) => {
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
    handleError(CALL_UNEXPECTED_ERROR, lowerString(message));
  });
  // let handle = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.insert(promise);
  return /* UniffiForeignFuture */ {
    handle: defaultUniffiHandle,
    free: uniffiForeignFutureFree,
  };
}

function uniffiForeignFutureFree(handle: UniffiHandle) {
  // UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.remove(handle);
}

// // For testing
// public func uniffiForeignFutureHandleCount{{ ci.namespace()|class_name(ci) }}() -> Int {
//     UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.count
// }
