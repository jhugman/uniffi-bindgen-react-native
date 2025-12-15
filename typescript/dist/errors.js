"use strict";
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.UniffiInternalError = exports.UniffiThrownObject = exports.UniffiError = void 0;
const symbols_1 = require("./symbols");
// The top level error class for all uniffi-wrapped errors.
//
// The readonly fields are used to implement both the instanceOf checks which are used
// in tests and in the generated callback code, and more locally the FfiConverters
// for each error.
class UniffiError extends Error {
    constructor(enumTypeName, variantName, message) {
        // We append the error type and variant to the message because we cannot override `toString()`—
        // in errors.test.ts, we see that the overridden `toString()` method is not called.
        super(createErrorMessage(enumTypeName, variantName, message));
    }
    static instanceOf(obj) {
        return obj[symbols_1.uniffiTypeNameSymbol] !== undefined && obj instanceof Error;
    }
}
exports.UniffiError = UniffiError;
function createErrorMessage(typeName, variantName, message) {
    const prefix = `${typeName}.${variantName}`;
    if (message) {
        return `${prefix}: ${message}`;
    }
    else {
        return prefix;
    }
}
class UniffiThrownObject extends Error {
    inner;
    static __baseTypeName = "UniffiThrownObject";
    __baseTypeName = UniffiThrownObject.__baseTypeName;
    constructor(typeName, inner, message) {
        // We append the error type and variant to the message because we cannot override `toString()`—
        // in errors.test.ts, we see that the overridden `toString()` method is not called.
        super(createObjectMessage(typeName, inner, message));
        this.inner = inner;
    }
    static instanceOf(err) {
        return (!!err &&
            err.__baseTypeName === UniffiThrownObject.__baseTypeName &&
            err instanceof Error);
    }
}
exports.UniffiThrownObject = UniffiThrownObject;
function createObjectMessage(typeName, obj, message) {
    return [typeName, stringRepresentation(obj), message]
        .filter((s) => !!s)
        .join(": ");
}
function stringRepresentation(obj) {
    if (obj.hasOwnProperty("toString") && typeof obj.toString === "function") {
        return obj.toString();
    }
    if (typeof obj.toDebugString === "function") {
        return obj.toDebugString();
    }
    return undefined;
}
exports.UniffiInternalError = (() => {
    class NumberOverflow extends Error {
        constructor() {
            super("Cannot convert a large BigInt into a number");
        }
    }
    class DateTimeOverflow extends Error {
        constructor() {
            super("Date overflowed passed maximum number of ms passed the epoch");
        }
    }
    class BufferOverflow extends Error {
        constructor() {
            super("Reading the requested value would read past the end of the buffer");
        }
    }
    class IncompleteData extends Error {
        constructor() {
            super("The buffer still has data after lifting its containing value");
        }
    }
    class AbortError extends Error {
        constructor() {
            super("A Rust future was aborted");
            this.name = "AbortError";
        }
    }
    class UnexpectedEnumCase extends Error {
        constructor() {
            super("Raw enum value doesn't match any cases");
        }
    }
    class UnexpectedNullPointer extends Error {
        constructor() {
            super("Raw pointer value was null");
        }
    }
    class UnexpectedRustCallStatusCode extends Error {
        constructor() {
            super("Unexpected UniffiRustCallStatus code");
        }
    }
    class UnexpectedRustCallError extends Error {
        constructor() {
            super("CALL_ERROR but no errorClass specified");
        }
    }
    class UnexpectedStaleHandle extends Error {
        constructor() {
            super("The object is no longer in the handle map, likely because of a hot-reload");
        }
    }
    class ContractVersionMismatch extends Error {
        constructor(rustVersion, bindingsVersion) {
            super(`Incompatible versions of uniffi were used to build the JS (${bindingsVersion}) from the Rust (${rustVersion})`);
        }
    }
    class ApiChecksumMismatch extends Error {
        constructor(func) {
            super(`FFI function ${func} has a checksum mismatch; this may signify previously undetected incompatible Uniffi versions`);
        }
    }
    class RustPanic extends Error {
        constructor(message) {
            super(message);
        }
    }
    class Unimplemented extends Error {
        constructor(message) {
            super(message);
        }
    }
    return {
        ApiChecksumMismatch,
        NumberOverflow,
        DateTimeOverflow,
        BufferOverflow,
        ContractVersionMismatch,
        IncompleteData,
        AbortError,
        UnexpectedEnumCase,
        UnexpectedNullPointer,
        UnexpectedRustCallStatusCode,
        UnexpectedRustCallError,
        UnexpectedStaleHandle,
        RustPanic,
        Unimplemented,
    };
})();
