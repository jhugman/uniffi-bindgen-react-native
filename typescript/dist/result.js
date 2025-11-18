"use strict";
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.UniffiResult = void 0;
exports.UniffiResult = {
    ready() {
        return { code: 0 };
    },
    writeError(result, code, buf) {
        const status = result;
        status.code = code;
        status.errorBuf = buf;
        return status;
    },
    writeSuccess(result, obj) {
        const refHolder = result;
        refHolder.pointee = obj;
        return refHolder;
    },
    success(pointee) {
        return { pointee };
    },
};
