import { type UniffiByteArray } from "./ffi-types";
import { type UniffiHandle } from "./handle-map";
import { type UniffiErrorHandler, type UniffiRustCallStatus, UniffiRustCaller } from "./rust-call";
export type UniffiRustFutureContinuationCallback = (handle: UniffiHandle, pollResult: number) => void;
type PollFunc = (rustFuture: bigint, cb: UniffiRustFutureContinuationCallback, handle: UniffiHandle) => void;
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
export declare function uniffiRustCallAsync<F, S extends UniffiRustCallStatus, T>(rustCaller: UniffiRustCaller<S>, rustFutureFunc: () => bigint, pollFunc: PollFunc, cancelFunc: (rustFuture: bigint) => void, completeFunc: (rustFuture: bigint, status: S) => F, freeFunc: (rustFuture: bigint) => void, liftFunc: (lower: F) => T, liftString: (bytes: UniffiByteArray) => string, asyncOpts?: {
    signal: AbortSignal;
}, errorHandler?: UniffiErrorHandler): Promise<T>;
export declare function uniffiRustFutureHandleCount(): number;
export {};
