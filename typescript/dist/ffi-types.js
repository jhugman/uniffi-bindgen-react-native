"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RustBuffer = void 0;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const errors_1 = require("./errors");
class RustBuffer {
    readOffset = 0;
    writeOffset = 0;
    capacity;
    arrayBuffer;
    constructor(arrayBuffer) {
        this.arrayBuffer = arrayBuffer;
        this.capacity = arrayBuffer.byteLength;
    }
    static withCapacity(capacity) {
        const buf = new ArrayBuffer(capacity);
        return new RustBuffer(buf);
    }
    static empty() {
        return this.withCapacity(0);
    }
    static fromArrayBuffer(buf) {
        return new RustBuffer(buf);
    }
    static fromByteArray(buf) {
        return new RustBuffer(buf.buffer);
    }
    get length() {
        return this.arrayBuffer.byteLength;
    }
    get byteArray() {
        return new Uint8Array(this.arrayBuffer);
    }
    readArrayBuffer(numBytes) {
        const start = this.readOffset;
        const end = this.checkOverflow(start, numBytes);
        const value = this.arrayBuffer.slice(start, end);
        this.readOffset = end;
        return value;
    }
    readByteArray(numBytes) {
        const start = this.readOffset;
        const end = this.checkOverflow(start, numBytes);
        const value = new Uint8Array(this.arrayBuffer, start, numBytes);
        this.readOffset = end;
        return value;
    }
    writeArrayBuffer(buffer) {
        const start = this.writeOffset;
        const end = this.checkOverflow(start, buffer.byteLength);
        const src = new Uint8Array(buffer);
        const dest = new Uint8Array(this.arrayBuffer, start);
        dest.set(src);
        this.writeOffset = end;
    }
    writeByteArray(src) {
        const start = this.writeOffset;
        const end = this.checkOverflow(start, src.byteLength);
        const dest = new Uint8Array(this.arrayBuffer, start);
        dest.set(src);
        this.writeOffset = end;
    }
    readWithView(numBytes, reader) {
        const start = this.readOffset;
        const end = this.checkOverflow(start, numBytes);
        const view = new DataView(this.arrayBuffer, start, numBytes);
        const value = reader(view);
        this.readOffset = end;
        return value;
    }
    writeWithView(numBytes, writer) {
        const start = this.writeOffset;
        const end = this.checkOverflow(start, numBytes);
        const view = new DataView(this.arrayBuffer, start, numBytes);
        writer(view);
        this.writeOffset = end;
    }
    checkOverflow(start, numBytes) {
        const end = start + numBytes;
        if (this.capacity < end) {
            throw new errors_1.UniffiInternalError.BufferOverflow();
        }
        return end;
    }
}
exports.RustBuffer = RustBuffer;
