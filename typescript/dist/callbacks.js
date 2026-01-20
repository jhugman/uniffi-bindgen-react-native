"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.FfiConverterCallback = void 0;
exports.uniffiTraitInterfaceCall = uniffiTraitInterfaceCall;
exports.uniffiTraitInterfaceCallWithError = uniffiTraitInterfaceCallWithError;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const ffi_converters_1 = require("./ffi-converters");
const handle_map_1 = require("./handle-map");
const rust_call_1 = require("./rust-call");
const handleConverter = ffi_converters_1.FfiConverterUInt64;
class FfiConverterCallback {
    handleMap;
    constructor(handleMap = new handle_map_1.UniffiHandleMap()) {
        this.handleMap = handleMap;
    }
    lift(value) {
        return this.handleMap.get(value);
    }
    lower(value) {
        return this.handleMap.insert(value);
    }
    read(from) {
        return this.lift(handleConverter.read(from));
    }
    write(value, into) {
        handleConverter.write(this.lower(value), into);
    }
    allocationSize(value) {
        return handleConverter.allocationSize(handle_map_1.defaultUniffiHandle);
    }
    drop(handle) {
        return this.handleMap.remove(handle);
    }
}
exports.FfiConverterCallback = FfiConverterCallback;
function uniffiTraitInterfaceCall(makeCall, handleSuccess, handleError, lowerString) {
    try {
        handleSuccess(makeCall());
    }
    catch (e) {
        handleError(rust_call_1.CALL_UNEXPECTED_ERROR, lowerString(e.toString()));
    }
}
function uniffiTraitInterfaceCallWithError(makeCall, handleSuccess, handleError, isErrorType, lowerError, lowerString) {
    try {
        handleSuccess(makeCall());
    }
    catch (e) {
        // Hermes' prototype chain seems buggy, so we need to make our
        // own arrangements
        if (isErrorType(e)) {
            handleError(rust_call_1.CALL_ERROR, lowerError(e));
        }
        else {
            handleError(rust_call_1.CALL_UNEXPECTED_ERROR, lowerString(e.toString()));
        }
    }
}
