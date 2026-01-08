"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.uniffiRustCallAsync = uniffiRustCallAsync;
exports.uniffiRustFutureHandleCount = uniffiRustFutureHandleCount;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const errors_1 = require("./errors");
const handle_map_1 = require("./handle-map");
const UNIFFI_RUST_FUTURE_POLL_READY = 0;
const UNIFFI_RUST_FUTURE_POLL_MAYBE_READY = 1;
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
async function uniffiRustCallAsync(rustCaller, rustFutureFunc, pollFunc, cancelFunc, completeFunc, freeFunc, liftFunc, liftString, asyncOpts, errorHandler) {
    // If the underlying Rust API supports task cancellation, then we should
    // check if should bail early.
    //
    // However, it's unlikely that the Rust API does; so this maybe the
    // only support we're giving that abort is supported.
    //
    // We'd like to use signal.throwIfAborted(), but the polyfill we use during
    // testing does not implement this method.
    if (asyncOpts?.signal.aborted === true) {
        return Promise.reject(new errors_1.UniffiInternalError.AbortError());
    }
    // This actually calls into the client rust method.
    const rustFuture = rustFutureFunc();
    const abortFunc = createAbortFunction(rustFuture, cancelFunc);
    asyncOpts?.signal.addEventListener("abort", abortFunc);
    // We now poll the Rust future until it's ready.
    // The poll, complete and free methods are specialized by the FFIType of the return value.
    try {
        let pollResult;
        do {
            // Calling pollFunc with a callback that resolves the promise that pollRust
            // returns: pollRust makes the promise, uniffiFutureContinuationCallback resolves it.
            pollResult = await pollRust((handle) => {
                pollFunc(rustFuture, uniffiFutureContinuationCallback, handle);
            });
        } while (pollResult !== UNIFFI_RUST_FUTURE_POLL_READY);
        // Now it's ready, all we need to do is pick up the result (and error).
        return liftFunc(rustCaller.makeRustCall((status) => completeFunc(rustFuture, status), liftString, errorHandler));
    }
    finally {
        // We remove the abortFunc now so we don't trigger a use-after-free
        // panic.
        asyncOpts?.signal.removeEventListener("abort", abortFunc);
        freeFunc(rustFuture);
    }
}
const UNIFFI_RUST_FUTURE_RESOLVER_MAP = new handle_map_1.UniffiHandleMap();
// pollRust makes a new promise, stores the resolver in the resolver map,
// then calls the pollFunc with the handle.
async function pollRust(pollFunc) {
    return new Promise((resolve) => {
        const handle = UNIFFI_RUST_FUTURE_RESOLVER_MAP.insert(resolve);
        pollFunc(handle);
    });
}
function createAbortFunction(rustFuture, cancelFunc) {
    // We don't do anything other than call cancel.
    // This will cause pollFunc to come back with a POLL_READY,
    // then the makeRustCall will throw an AbortError.
    return () => cancelFunc(rustFuture);
}
// Rust calls this callback, which resolves the promise returned by pollRust.
const uniffiFutureContinuationCallback = (handle, pollResult) => {
    const resolve = UNIFFI_RUST_FUTURE_RESOLVER_MAP.remove(handle);
    // From https://github.com/mozilla/uniffi-rs/pull/1837/files#diff-8a28c9cf1245b4f714d406ea4044d68e1000099928eaca1afb504ccbc008fe9fR35-R37
    //
    // > WARNING: the call to [rust_future_poll] must be scheduled to happen soon after the callback is
    // > called, but not inside the callback itself.  If [rust_future_poll] is called inside the
    // > callback, some futures will deadlock and our scheduler code might as well.
    //
    // We avoid this by using UniffiCallInvoker::invokeNonBlocking for this callback.
    if (resolve) {
        resolve(pollResult);
    }
};
// For testing only.
function uniffiRustFutureHandleCount() {
    return UNIFFI_RUST_FUTURE_RESOLVER_MAP.size;
}
