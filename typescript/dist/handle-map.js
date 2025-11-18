"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.UniffiHandleMap = exports.defaultUniffiHandle = void 0;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const errors_1 = require("./errors");
exports.defaultUniffiHandle = BigInt("0");
class UniffiHandleMap {
    map = new Map();
    currentHandle = exports.defaultUniffiHandle;
    insert(value) {
        this.map.set(this.currentHandle, value);
        return this.currentHandle++;
    }
    get(handle) {
        const obj = this.map.get(handle);
        if (obj === undefined) {
            // Rust is holding a handle which is no longer in the handle map, either
            // because this is a different handle map to the one it was inserted in,
            // or that the handle has already been removed from the handlemap it was
            // originally in.
            //
            // This is because of either:
            //   a) the Typescript has changed state without resetting a callback
            //      interface, i.e. a hot reload.
            //   b) a bug in uniffi-bindgen-react-native.
            //
            // If this error is thrown when the app is in the wild, i.e. outside of a
            // development, i.e. not a hot reload, then please file a bug with
            // uniffi-bindgen-react-native.
            //
            // Otherwise, this error is not recoverable, and a cold reload is
            // necessary.
            //
            // If the error is not intermittent, i.e. happening every reload, then
            // you should probably consider changing the Rust library to not hold
            // on to callback interfaces and foreign trait instances across reloads,
            // e.g. creating app or page lifecycle API, or replacing rather than
            // appending listeners.
            throw new errors_1.UniffiInternalError.UnexpectedStaleHandle();
        }
        return obj;
    }
    remove(handle) {
        const obj = this.map.get(handle);
        if (obj !== undefined) {
            this.map.delete(handle);
        }
        return obj;
    }
    has(handle) {
        return this.map.has(handle);
    }
    get size() {
        return this.map.size;
    }
}
exports.UniffiHandleMap = UniffiHandleMap;
