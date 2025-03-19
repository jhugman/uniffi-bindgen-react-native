/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";
import { type UniffiByteArray } from "./ffi-types";

export const CALL_SUCCESS = 0;
export const CALL_ERROR = 1;
export const CALL_UNEXPECTED_ERROR = 2;
export const CALL_CANCELLED = 3;

type StringLifter = (bytes: UniffiByteArray) => string;
const emptyStringLifter = (bytes: UniffiByteArray) =>
  "An error occurred decoding a string";

export type UniffiRustCallStatus = {
  code: number;
  errorBuf?: UniffiByteArray;
};
export class UniffiRustCaller {
  constructor(
    private statusConstructor: () => UniffiRustCallStatus = uniffiCreateCallStatus,
  ) {}

  rustCall<T>(
    caller: RustCallFn<T>,
    liftString: StringLifter = emptyStringLifter,
  ): T {
    return this.makeRustCall(caller, liftString);
  }

  rustCallWithError<T>(
    errorHandler: UniffiErrorHandler,
    caller: RustCallFn<T>,
    liftString: StringLifter = emptyStringLifter,
  ): T {
    return this.makeRustCall(caller, liftString, errorHandler);
  }

  createCallStatus(): UniffiRustCallStatus {
    return this.statusConstructor();
  }

  createErrorStatus(
    code: number,
    errorBuf: UniffiByteArray,
  ): UniffiRustCallStatus {
    const status = this.statusConstructor();
    status.code = code;
    status.errorBuf = errorBuf;
    return status;
  }

  makeRustCall<T>(
    caller: RustCallFn<T>,
    liftString: StringLifter,
    errorHandler?: UniffiErrorHandler,
  ): T {
    const callStatus = this.statusConstructor();
    let returnedVal = caller(callStatus);
    uniffiCheckCallStatus(callStatus, liftString, errorHandler);
    return returnedVal;
  }
}

function uniffiCreateCallStatus(): UniffiRustCallStatus {
  return { code: CALL_SUCCESS };
}

export type UniffiErrorHandler = (buffer: UniffiByteArray) => Error;
type RustCallFn<T> = (status: UniffiRustCallStatus) => T;

function uniffiCheckCallStatus(
  callStatus: UniffiRustCallStatus,
  liftString: StringLifter,
  errorHandler?: UniffiErrorHandler,
) {
  switch (callStatus.code) {
    case CALL_SUCCESS:
      return;

    case CALL_ERROR: {
      const errorBuf = callStatus.errorBuf;
      if (errorBuf) {
        if (errorHandler) {
          throw errorHandler(errorBuf);
        }
      }
      throw new UniffiInternalError.UnexpectedRustCallError();
    }

    case CALL_UNEXPECTED_ERROR: {
      // When the rust code sees a panic, it tries to construct a RustBuffer
      // with the message.  But if that code panics, then it just sends back
      // an empty buffer.
      const errorBuf = callStatus.errorBuf;
      if (errorBuf) {
        if (errorBuf.byteLength > 0) {
          throw new UniffiInternalError.RustPanic(liftString(errorBuf));
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
      // As of uniffi-rs v0.28.0, call cancellation is only checked for in the Swift bindings,
      // and uses an Unimplemeneted error.
      throw new UniffiInternalError.AbortError();

    default:
      throw new UniffiInternalError.UnexpectedRustCallStatusCode();
  }
}

/**
 * A member of any object, backed by a C++ DestructibleObject (@see RustArcPtr.h).
 *
 * The object has a destructor lambda.
 */
export type UniffiRustArcPtr = {
  /**
   * Called by the `object.uniffiDestroy()` to disable the
   * action of the C++ destructor.
   */
  markDestroyed(): void;
};
