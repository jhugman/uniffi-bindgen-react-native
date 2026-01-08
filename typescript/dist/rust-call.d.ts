import { type UniffiByteArray } from "./ffi-types";
export declare const CALL_SUCCESS = 0;
export declare const CALL_ERROR = 1;
export declare const CALL_UNEXPECTED_ERROR = 2;
export declare const CALL_CANCELLED = 3;
type StringLifter = (bytes: UniffiByteArray) => string;
export type UniffiRustCallStatus = {
    code: number;
    errorBuf?: UniffiByteArray;
};
export declare class UniffiRustCaller<Status extends UniffiRustCallStatus> {
    private statusConstructor;
    constructor(statusConstructor: () => Status);
    rustCall<T>(caller: RustCallFn<Status, T>, liftString?: StringLifter): T;
    rustCallWithError<T>(errorHandler: UniffiErrorHandler, caller: RustCallFn<Status, T>, liftString?: StringLifter): T;
    createCallStatus(): Status;
    createErrorStatus(code: number, errorBuf: UniffiByteArray): Status;
    makeRustCall<T>(caller: RustCallFn<Status, T>, liftString: StringLifter, errorHandler?: UniffiErrorHandler): T;
}
export type UniffiErrorHandler = (buffer: UniffiByteArray) => Error;
type RustCallFn<S, T> = (status: S) => T;
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
export {};
