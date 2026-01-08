"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.UniffiRustCaller = exports.CALL_CANCELLED = exports.CALL_UNEXPECTED_ERROR = exports.CALL_ERROR = exports.CALL_SUCCESS = void 0;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const errors_1 = require("./errors");
exports.CALL_SUCCESS = 0;
exports.CALL_ERROR = 1;
exports.CALL_UNEXPECTED_ERROR = 2;
exports.CALL_CANCELLED = 3;
const emptyStringLifter = (bytes) => "An error occurred decoding a string";
class UniffiRustCaller {
    statusConstructor;
    constructor(statusConstructor) {
        this.statusConstructor = statusConstructor;
    }
    rustCall(caller, liftString = emptyStringLifter) {
        return this.makeRustCall(caller, liftString);
    }
    rustCallWithError(errorHandler, caller, liftString = emptyStringLifter) {
        return this.makeRustCall(caller, liftString, errorHandler);
    }
    createCallStatus() {
        return this.statusConstructor();
    }
    createErrorStatus(code, errorBuf) {
        const status = this.statusConstructor();
        status.code = code;
        status.errorBuf = errorBuf;
        return status;
    }
    makeRustCall(caller, liftString, errorHandler) {
        const callStatus = this.statusConstructor();
        let returnedVal = caller(callStatus);
        uniffiCheckCallStatus(callStatus, liftString, errorHandler);
        return returnedVal;
    }
}
exports.UniffiRustCaller = UniffiRustCaller;
function uniffiCreateCallStatus() {
    return { code: exports.CALL_SUCCESS };
}
function uniffiCheckCallStatus(callStatus, liftString, errorHandler) {
    switch (callStatus.code) {
        case exports.CALL_SUCCESS:
            return;
        case exports.CALL_ERROR: {
            const errorBuf = callStatus.errorBuf;
            if (errorBuf) {
                if (errorHandler) {
                    throw errorHandler(errorBuf);
                }
            }
            throw new errors_1.UniffiInternalError.UnexpectedRustCallError();
        }
        case exports.CALL_UNEXPECTED_ERROR: {
            // When the rust code sees a panic, it tries to construct a RustBuffer
            // with the message.  But if that code panics, then it just sends back
            // an empty buffer.
            const errorBuf = callStatus.errorBuf;
            if (errorBuf) {
                if (errorBuf.byteLength > 0) {
                    throw new errors_1.UniffiInternalError.RustPanic(liftString(errorBuf));
                }
            }
            throw new errors_1.UniffiInternalError.RustPanic("Rust panic");
        }
        case exports.CALL_CANCELLED:
            // #RUST_TASK_CANCELLATION:
            //
            // This error code is expected when a Rust Future is cancelled or aborted, either
            // from the foreign side, or from within Rust itself.
            //
            // As of uniffi-rs v0.28.0, call cancellation is only checked for in the Swift bindings,
            // and uses an Unimplemeneted error.
            throw new errors_1.UniffiInternalError.AbortError();
        default:
            throw new errors_1.UniffiInternalError.UnexpectedRustCallStatusCode();
    }
}
