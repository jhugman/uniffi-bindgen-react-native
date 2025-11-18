"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.uniffiTraitInterfaceCallAsync = uniffiTraitInterfaceCallAsync;
exports.uniffiTraitInterfaceCallAsyncWithError = uniffiTraitInterfaceCallAsyncWithError;
exports.uniffiForeignFutureHandleCount = uniffiForeignFutureHandleCount;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const rust_call_1 = require("./rust-call");
const handle_map_1 = require("./handle-map");
const UNIFFI_FOREIGN_FUTURE_HANDLE_MAP = new handle_map_1.UniffiHandleMap();
// Some degenerate functions used for default arguments.
const notExpectedError = (err) => false;
function emptyLowerError(e) {
    throw new Error("Unreachable");
}
function uniffiTraitInterfaceCallAsync(makeCall, handleSuccess, handleError, lowerString) {
    return uniffiTraitInterfaceCallAsyncWithError(makeCall, handleSuccess, handleError, notExpectedError, emptyLowerError, lowerString);
}
function uniffiTraitInterfaceCallAsyncWithError(makeCall, handleSuccess, handleError, isErrorType, lowerError, lowerString) {
    const settledHolder = { settled: false };
    const abortController = new AbortController();
    const promise = makeCall(abortController.signal)
        // Before doing anything else, we record that the promise has been settled.
        // Doing this after the `then` call means we only do this once all of that has finished,
        // which is way too late.
        .finally(() => (settledHolder.settled = true))
        .then(handleSuccess, (error) => {
        let message = error.message ? error.message : error.toString();
        if (isErrorType(error)) {
            try {
                handleError(rust_call_1.CALL_ERROR, lowerError(error));
                return;
            }
            catch (e) {
                // Fall through to unexpected error handling below.
                message = `Error handling error "${e}", originally: "${message}"`;
            }
        }
        // This is the catch all:
        // 1. if there was an unexpected error causing a rejection
        // 2. if there was an unexpected error in the handleError function.
        handleError(rust_call_1.CALL_UNEXPECTED_ERROR, lowerString(message));
    });
    const promiseHelper = { abortController, settledHolder, promise };
    const handle = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.insert(promiseHelper);
    return /* UniffiForeignFuture */ {
        handle,
        free: uniffiForeignFutureFree,
    };
}
function uniffiForeignFutureFree(handle) {
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
function uniffiForeignFutureHandleCount() {
    return UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.size;
}
