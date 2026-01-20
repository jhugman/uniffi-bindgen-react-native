"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.FfiConverterArrayBuffer = exports.FfiConverterMap = exports.FfiConverterArray = exports.FfiConverterOptional = exports.FfiConverterTimestamp = exports.FfiConverterDuration = exports.FfiConverterBool = exports.FfiConverterUInt64 = exports.FfiConverterUInt32 = exports.FfiConverterUInt16 = exports.FfiConverterUInt8 = exports.FfiConverterFloat64 = exports.FfiConverterFloat32 = exports.FfiConverterInt64 = exports.FfiConverterInt32 = exports.FfiConverterInt16 = exports.FfiConverterInt8 = exports.AbstractFfiConverterByteArray = exports.FfiConverterPrimitive = void 0;
exports.uniffiCreateFfiConverterString = uniffiCreateFfiConverterString;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const errors_1 = require("./errors");
const ffi_types_1 = require("./ffi-types");
class FfiConverterPrimitive {
    lift(value) {
        return value;
    }
    lower(value) {
        return value;
    }
}
exports.FfiConverterPrimitive = FfiConverterPrimitive;
class AbstractFfiConverterByteArray {
    lift(value) {
        const buffer = ffi_types_1.RustBuffer.fromByteArray(value);
        return this.read(buffer);
    }
    lower(value) {
        const buffer = ffi_types_1.RustBuffer.withCapacity(this.allocationSize(value));
        this.write(value, buffer);
        return buffer.byteArray;
    }
}
exports.AbstractFfiConverterByteArray = AbstractFfiConverterByteArray;
class FfiConverterNumber extends FfiConverterPrimitive {
    reader;
    writer;
    byteSize;
    // These fields should be private, but Typescript doesn't allow
    // that because of the way they are exposed.
    constructor(reader, writer, byteSize) {
        super();
        this.reader = reader;
        this.writer = writer;
        this.byteSize = byteSize;
    }
    read(from) {
        return from.readWithView(this.byteSize, this.reader);
    }
    write(value, into) {
        return into.writeWithView(this.byteSize, (view) => this.writer(view, value));
    }
    allocationSize(value) {
        return this.byteSize;
    }
}
const littleEndian = false;
// Ints
exports.FfiConverterInt8 = new FfiConverterNumber((view) => view.getInt8(0), (view, value) => view.setInt8(0, value), Int8Array.BYTES_PER_ELEMENT);
exports.FfiConverterInt16 = new FfiConverterNumber((view) => view.getInt16(0, littleEndian), (view, value) => view.setInt16(0, value, littleEndian), Int16Array.BYTES_PER_ELEMENT);
exports.FfiConverterInt32 = new FfiConverterNumber((view) => view.getInt32(0, littleEndian), (view, value) => view.setInt32(0, value, littleEndian), Int32Array.BYTES_PER_ELEMENT);
exports.FfiConverterInt64 = new FfiConverterNumber((view) => view.getBigInt64(0, littleEndian), (view, value) => view.setBigInt64(0, value, littleEndian), BigInt64Array.BYTES_PER_ELEMENT);
// Floats
exports.FfiConverterFloat32 = new FfiConverterNumber((view) => view.getFloat32(0, littleEndian), (view, value) => view.setFloat32(0, value, littleEndian), Float32Array.BYTES_PER_ELEMENT);
exports.FfiConverterFloat64 = new FfiConverterNumber((view) => view.getFloat64(0, littleEndian), (view, value) => view.setFloat64(0, value, littleEndian), Float64Array.BYTES_PER_ELEMENT);
// UInts
exports.FfiConverterUInt8 = new FfiConverterNumber((view) => view.getUint8(0), (view, value) => view.setUint8(0, value), Uint8Array.BYTES_PER_ELEMENT);
exports.FfiConverterUInt16 = new FfiConverterNumber((view) => view.getUint16(0, littleEndian), (view, value) => view.setUint16(0, value, littleEndian), Uint16Array.BYTES_PER_ELEMENT);
exports.FfiConverterUInt32 = new FfiConverterNumber((view) => view.getUint32(0, littleEndian), (view, value) => view.setUint32(0, value, littleEndian), Uint32Array.BYTES_PER_ELEMENT);
exports.FfiConverterUInt64 = new FfiConverterNumber((view) => view.getBigUint64(0, littleEndian), (view, value) => view.setBigUint64(0, value, littleEndian), BigUint64Array.BYTES_PER_ELEMENT);
// Bool
exports.FfiConverterBool = (() => {
    const byteConverter = exports.FfiConverterInt8;
    class FfiConverterBool {
        lift(value) {
            return !!value;
        }
        lower(value) {
            return value ? 1 : 0;
        }
        read(from) {
            return this.lift(byteConverter.read(from));
        }
        write(value, into) {
            byteConverter.write(this.lower(value), into);
        }
        allocationSize(value) {
            return byteConverter.allocationSize(0);
        }
    }
    return new FfiConverterBool();
})();
exports.FfiConverterDuration = (() => {
    const secondsConverter = exports.FfiConverterUInt64;
    const nanosConverter = exports.FfiConverterUInt32;
    const msPerSecBigInt = BigInt("1000");
    const nanosPerMs = 1e6;
    class FFIConverter extends AbstractFfiConverterByteArray {
        read(from) {
            const secsBigInt = secondsConverter.read(from);
            const nanos = nanosConverter.read(from);
            const ms = Number(secsBigInt * msPerSecBigInt);
            if (ms === Number.POSITIVE_INFINITY || ms === Number.NEGATIVE_INFINITY) {
                throw new errors_1.UniffiInternalError.NumberOverflow();
            }
            return ms + nanos / nanosPerMs;
        }
        write(value, into) {
            const ms = value.valueOf();
            const secsBigInt = BigInt(Math.trunc(ms)) / msPerSecBigInt;
            const remainingNanos = (ms % 1000) * nanosPerMs;
            secondsConverter.write(secsBigInt, into);
            nanosConverter.write(remainingNanos, into);
        }
        allocationSize(_value) {
            return (secondsConverter.allocationSize(msPerSecBigInt) +
                nanosConverter.allocationSize(0));
        }
    }
    return new FFIConverter();
})();
exports.FfiConverterTimestamp = (() => {
    const secondsConverter = exports.FfiConverterInt64;
    const nanosConverter = exports.FfiConverterUInt32;
    const msPerSecBigInt = BigInt("1000");
    const nanosPerMs = 1e6;
    const msPerSec = 1e3;
    const maxMsFromEpoch = 8.64e15;
    function safeDate(ms) {
        if (Math.abs(ms) > 8.64e15) {
            throw new errors_1.UniffiInternalError.DateTimeOverflow();
        }
        return new Date(ms);
    }
    class FFIConverter extends AbstractFfiConverterByteArray {
        read(from) {
            const secsBigInt = secondsConverter.read(from);
            const nanos = nanosConverter.read(from);
            const ms = Number(secsBigInt * msPerSecBigInt);
            if (ms >= 0) {
                return safeDate(ms + nanos / nanosPerMs);
            }
            else {
                return safeDate(ms - nanos / nanosPerMs);
            }
        }
        write(value, into) {
            const ms = value.valueOf();
            const secsBigInt = BigInt(Math.trunc(ms / msPerSec));
            const remainingNanos = Math.abs((ms % msPerSec) * nanosPerMs);
            secondsConverter.write(secsBigInt, into);
            nanosConverter.write(remainingNanos, into);
        }
        allocationSize(_value) {
            return (secondsConverter.allocationSize(msPerSecBigInt) +
                nanosConverter.allocationSize(0));
        }
    }
    return new FFIConverter();
})();
class FfiConverterOptional extends AbstractFfiConverterByteArray {
    itemConverter;
    static flagConverter = exports.FfiConverterBool;
    constructor(itemConverter) {
        super();
        this.itemConverter = itemConverter;
    }
    read(from) {
        const flag = FfiConverterOptional.flagConverter.read(from);
        return flag ? this.itemConverter.read(from) : undefined;
    }
    write(value, into) {
        if (value !== undefined) {
            FfiConverterOptional.flagConverter.write(true, into);
            this.itemConverter.write(value, into);
        }
        else {
            FfiConverterOptional.flagConverter.write(false, into);
        }
    }
    allocationSize(value) {
        let size = FfiConverterOptional.flagConverter.allocationSize(true);
        if (value !== undefined) {
            size += this.itemConverter.allocationSize(value);
        }
        return size;
    }
}
exports.FfiConverterOptional = FfiConverterOptional;
class FfiConverterArray extends AbstractFfiConverterByteArray {
    itemConverter;
    static sizeConverter = exports.FfiConverterInt32;
    constructor(itemConverter) {
        super();
        this.itemConverter = itemConverter;
    }
    read(from) {
        const size = FfiConverterArray.sizeConverter.read(from);
        const array = new Array(size);
        for (let i = 0; i < size; i++) {
            array[i] = this.itemConverter.read(from);
        }
        return array;
    }
    write(array, into) {
        FfiConverterArray.sizeConverter.write(array.length, into);
        for (const item of array) {
            this.itemConverter.write(item, into);
        }
    }
    allocationSize(array) {
        let size = FfiConverterArray.sizeConverter.allocationSize(array.length);
        for (const item of array) {
            size += this.itemConverter.allocationSize(item);
        }
        return size;
    }
}
exports.FfiConverterArray = FfiConverterArray;
class FfiConverterMap extends AbstractFfiConverterByteArray {
    keyConverter;
    valueConverter;
    static sizeConverter = exports.FfiConverterInt32;
    constructor(keyConverter, valueConverter) {
        super();
        this.keyConverter = keyConverter;
        this.valueConverter = valueConverter;
    }
    read(from) {
        const size = FfiConverterMap.sizeConverter.read(from);
        const map = new Map();
        for (let i = 0; i < size; i++) {
            map.set(this.keyConverter.read(from), this.valueConverter.read(from));
        }
        return map;
    }
    write(map, into) {
        FfiConverterMap.sizeConverter.write(map.size, into);
        for (const [k, v] of map.entries()) {
            this.keyConverter.write(k, into);
            this.valueConverter.write(v, into);
        }
    }
    allocationSize(map) {
        let size = FfiConverterMap.sizeConverter.allocationSize(map.size);
        for (const [k, v] of map.entries()) {
            size +=
                this.keyConverter.allocationSize(k) +
                    this.valueConverter.allocationSize(v);
        }
        return size;
    }
}
exports.FfiConverterMap = FfiConverterMap;
exports.FfiConverterArrayBuffer = (() => {
    const lengthConverter = exports.FfiConverterInt32;
    class FFIConverter extends AbstractFfiConverterByteArray {
        read(from) {
            const length = lengthConverter.read(from);
            return from.readArrayBuffer(length);
        }
        write(value, into) {
            const length = value.byteLength;
            lengthConverter.write(length, into);
            into.writeByteArray(new Uint8Array(value));
        }
        allocationSize(value) {
            return lengthConverter.allocationSize(0) + value.byteLength;
        }
    }
    return new FFIConverter();
})();
function uniffiCreateFfiConverterString(converter) {
    const lengthConverter = exports.FfiConverterInt32;
    class FFIConverter {
        lift(value) {
            return converter.bytesToString(value);
        }
        lower(value) {
            return converter.stringToBytes(value);
        }
        read(from) {
            const length = lengthConverter.read(from);
            // TODO Currently, RustBufferHelper.cpp is pretty dumb,
            // and copies all the bytes in the underlying ArrayBuffer.
            // Making a better shim for Uint8Array would allow us to use
            // readByteArray here, and eliminate a copy.
            const bytes = from.readArrayBuffer(length);
            return converter.bytesToString(new Uint8Array(bytes));
        }
        write(value, into) {
            // TODO: work on RustBufferHelper.cpp is needed to avoid
            // the extra copy and use writeByteArray.
            const buffer = converter.stringToBytes(value).buffer;
            const numBytes = buffer.byteLength;
            lengthConverter.write(numBytes, into);
            into.writeArrayBuffer(buffer);
        }
        allocationSize(value) {
            return (lengthConverter.allocationSize(0) + converter.stringByteLength(value));
        }
    }
    return new FFIConverter();
}
