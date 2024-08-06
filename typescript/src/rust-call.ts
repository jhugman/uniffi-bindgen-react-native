/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

export const CALL_SUCCESS = 0;
export const CALL_ERROR = 1;
export const CALL_UNEXPECTED_ERROR = 2;
export const CALL_CANCELLED = 3;

type StringLifter = (arrayBuffer: ArrayBuffer) => string;
const emptyStringLifter = (arrayBuffer: ArrayBuffer) =>
  "An error occurred decoding a string";

export type UniffiRustCallStatus = {
  code: number;
  errorBuf?: ArrayBuffer;
};
export function uniffiCreateCallStatus(): UniffiRustCallStatus {
  return { code: CALL_SUCCESS };
}

export type UniffiErrorHandler = (buffer: ArrayBuffer) => Error;
type RustCaller<T> = (status: UniffiRustCallStatus) => T;

export function rustCall<T>(
  caller: RustCaller<T>,
  liftString: StringLifter = emptyStringLifter,
): T {
  return makeRustCall(caller, liftString);
}

export function rustCallWithError<T>(
  errorHandler: UniffiErrorHandler,
  caller: RustCaller<T>,
  liftString: StringLifter = emptyStringLifter,
): T {
  return makeRustCall(caller, liftString, errorHandler);
}

export function makeRustCall<T>(
  caller: RustCaller<T>,
  liftString: StringLifter,
  errorHandler?: UniffiErrorHandler,
): T {
  // uniffiEnsureInitialized()
  const callStatus = uniffiCreateCallStatus();
  let returnedVal = caller(callStatus);
  uniffiCheckCallStatus(callStatus, liftString, errorHandler);
  return returnedVal;
}

function uniffiCheckCallStatus(
  callStatus: UniffiRustCallStatus,
  liftString: StringLifter,
  errorHandler?: UniffiErrorHandler,
) {
  switch (callStatus.code) {
    case CALL_SUCCESS:
      return;

    case CALL_ERROR: {
      if (callStatus.errorBuf) {
        if (errorHandler) {
          throw errorHandler(callStatus.errorBuf);
        }
      }
      throw new UniffiInternalError.UnexpectedRustCallError();
    }

    case CALL_UNEXPECTED_ERROR: {
      // When the rust code sees a panic, it tries to construct a RustBuffer
      // with the message.  But if that code panics, then it just sends back
      // an empty buffer.
      if (callStatus.errorBuf) {
        if (callStatus.errorBuf.byteLength > 0) {
          throw new UniffiInternalError.RustPanic(
            liftString(callStatus.errorBuf),
          );
        }
      }
      throw new UniffiInternalError.RustPanic("Rust panic");
    }

    case CALL_CANCELLED:
      // #RUST_TASK_CANCELLATION:
      //
      // This error code is expected when a Rust Future is cancelled or aborted, either
      // from the foreign side, or from within Rust itself.
      //
      // It's expected that this would be encountered when the `completeFunc` is called
      // from `uniffiRustCallAsync`.
      //
      // Currently, there is no generated API of cancelling a promise from JS.
      //
      // As of uniffi-rs v0.28.0, call cancellation is only checked for in the Swift bindings,
      // and has a similar Unimplemeneted error.
      throw new UniffiInternalError.Unimplemented(
        "Cancellation not supported yet",
      );

    default:
      throw new UniffiInternalError.UnexpectedRustCallStatusCode();
  }
}

export type UniffiRustArcPtrDestructor = (pointer: bigint) => void;

export type UniffiRustArcPtr = {
  markDestroyed(): void;
};
