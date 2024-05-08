import { UniffiInternalError } from "./errors";
import { RustBuffer } from "./ffi-types";

const CALL_SUCCESS = 0;
const CALL_ERROR = 1;
const CALL_UNEXPECTED_ERROR = 2;
const CALL_CANCELLED = 3;

type StringLifter = (arrayBuffer: ArrayBuffer) => string;
let FfiConverterString_lift: StringLifter;
export function initializeWithStringLifter(sr: StringLifter) {
  FfiConverterString_lift = sr;
}

export type UniffiRustCallStatus = {
  code: number;
  errorBuf?: ArrayBuffer;
};
export function createCallStatus(): UniffiRustCallStatus {
  return { code: CALL_SUCCESS };
}

type ErrorHandler = (buffer: ArrayBuffer) => Error;
type RustCaller<T> = (status: UniffiRustCallStatus) => T;

export function rustCall<T>(caller: RustCaller<T>): T {
  return makeRustCall(caller);
}

export function rustCallWithError<T>(
  errorHandler: ErrorHandler,
  caller: RustCaller<T>,
): T {
  return makeRustCall(caller, errorHandler);
}

export function makeRustCall<T>(
  caller: RustCaller<T>,
  errorHandler?: ErrorHandler,
): T {
  // uniffiEnsureInitialized()
  const callStatus = createCallStatus();
  let returnedVal = caller(callStatus);
  uniffiCheckCallStatus(callStatus, errorHandler);
  return returnedVal;
}

function uniffiCheckCallStatus(
  callStatus: UniffiRustCallStatus,
  errorHandler?: ErrorHandler,
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
            FfiConverterString_lift(callStatus.errorBuf),
          );
        }
      }
      throw new UniffiInternalError.RustPanic("Rust panic");
    }

    case CALL_CANCELLED:
      new UniffiInternalError.Unimplemented("Cancellation not supported yet");

    default:
      throw new UniffiInternalError.UnexpectedRustCallStatusCode();
  }
}

export type UniffiRustFutureContinuationCallback = () => void;
