/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { Cursor } from "./cursor.ts";
import { UniffiInternalError } from "./errors.ts";
import { type UniffiByteArray } from "./ffi-types.ts";

// https://github.com/mozilla/uniffi-rs/blob/main/docs/manual/src/internals/lifting_and_lowering.md
//
// `lower(value, alloc)` takes an allocator the caller controls. Only
// byte-array-backed converters (RustBuffer payloads) actually use it; primitive
// converters ignore it. Centralising the parameter on the interface keeps
// codegen call sites uniform.
export type RustBufferAllocator = (n: number) => Uint8Array;

export interface FfiConverter<FfiType, TsType> {
  lift(value: FfiType): TsType;
  lower(value: TsType, alloc: RustBufferAllocator): FfiType;
  readFromCursor(c: Cursor): TsType;
  writeIntoCursor(value: TsType, c: Cursor): void;
  allocationSize(value: TsType): number;
}

export abstract class AbstractFfiConverterByteArray<TsType>
  implements FfiConverter<UniffiByteArray, TsType>
{
  lift(value: UniffiByteArray): TsType {
    const c = Cursor.fromUint8Array(value);
    return this.readFromCursor(c);
  }
  lower(value: TsType, alloc: RustBufferAllocator): UniffiByteArray {
    const capacity = this.allocationSize(value);
    const view = alloc(capacity);
    const c = Cursor.fromUint8Array(view);
    this.writeIntoCursor(value, c);
    const written = (c as any).pos;
    if (written === capacity) {
      // Exact sizing — every converter except strings knows its size up front.
      return view;
    }
    // A string was sized by upper bound, so the tail of the allocation is
    // slack. Shrink the view to the message bytes and record the real
    // allocation size, which `rustbuffer_free` needs to free correctly.
    const exact = view.subarray(0, written);
    (exact as any).__ubrnRustCapacity = capacity;
    return exact;
  }

  abstract readFromCursor(c: Cursor): TsType;
  abstract writeIntoCursor(value: TsType, c: Cursor): void;
  abstract allocationSize(value: TsType): number;
}

/**
 * Build a primitive `FfiConverter` whose hot path delegates straight into
 * Cursor's monomorphic typed accessors. The returned object is a plain
 * module-level value (not a class instance) — V8 can inline `lift`/`lower`
 * trivially, and `readFromCursor`/`writeIntoCursor` are direct references
 * to the Cursor method wrappers (no per-call function-pointer dispatch via
 * the old `FfiConverterNumber(reader, writer, byteSize)` closure shape).
 */
function makePrimitive<T>(
  read: (c: Cursor) => T,
  write: (v: T, c: Cursor) => void,
  byteSize: number,
): FfiConverter<T, T> {
  return {
    lift: (v: T) => v,
    lower: (v: T, _alloc: RustBufferAllocator) => v,
    allocationSize: (_v: T) => byteSize,
    readFromCursor: read,
    writeIntoCursor: write,
  };
}

// Ints
export const FfiConverterInt8 = makePrimitive<number>(
  (c) => c.readI8(),
  (v, c) => c.writeI8(v),
  1,
);
export const FfiConverterInt16 = makePrimitive<number>(
  (c) => c.readI16(),
  (v, c) => c.writeI16(v),
  2,
);
export const FfiConverterInt32 = makePrimitive<number>(
  (c) => c.readI32(),
  (v, c) => c.writeI32(v),
  4,
);
export const FfiConverterInt64 = makePrimitive<bigint>(
  (c) => c.readI64(),
  (v, c) => c.writeI64(v),
  8,
);

// Floats
export const FfiConverterFloat32 = makePrimitive<number>(
  (c) => c.readF32(),
  (v, c) => c.writeF32(v),
  4,
);
export const FfiConverterFloat64 = makePrimitive<number>(
  (c) => c.readF64(),
  (v, c) => c.writeF64(v),
  8,
);

// UInts
export const FfiConverterUInt8 = makePrimitive<number>(
  (c) => c.readU8(),
  (v, c) => c.writeU8(v),
  1,
);
export const FfiConverterUInt16 = makePrimitive<number>(
  (c) => c.readU16(),
  (v, c) => c.writeU16(v),
  2,
);
export const FfiConverterUInt32 = makePrimitive<number>(
  (c) => c.readU32(),
  (v, c) => c.writeU32(v),
  4,
);
export const FfiConverterUInt64 = makePrimitive<bigint>(
  (c) => c.readU64(),
  (v, c) => c.writeU64(v),
  8,
);

// Bool — separate from `makePrimitive` because lift/lower convert between
// the on-the-wire `number` and JS `boolean` rather than passing through.
export const FfiConverterBool = (() => {
  const read = (c: Cursor) => c.readBool();
  const write = (v: boolean, c: Cursor) => c.writeBool(v);
  return {
    lift: (n: number) => !!n,
    lower: (b: boolean, _alloc: RustBufferAllocator) => (b ? 1 : 0),
    allocationSize: (_v: boolean) => 1,
    readFromCursor: read,
    writeIntoCursor: write,
  } satisfies FfiConverter<number, boolean>;
})();

// Duration
//
// There is currently no JS API for duration, so we'll make this just milliseconds.
//
// Later on we'll need to put a Temporal based converter,
// and switch on from a config file.
export type UniffiDuration = number;
export const FfiConverterDuration = (() => {
  const msPerSecBigInt = BigInt("1000");
  const nanosPerMs = 1e6;
  class FFIConverter extends AbstractFfiConverterByteArray<UniffiDuration> {
    readFromCursor(c: Cursor): UniffiDuration {
      const secsBigInt = c.readU64();
      const nanos = c.readU32();
      const ms = Number(secsBigInt * msPerSecBigInt);
      if (ms === Number.POSITIVE_INFINITY || ms === Number.NEGATIVE_INFINITY) {
        throw new UniffiInternalError.NumberOverflow();
      }
      return ms + nanos / nanosPerMs;
    }
    writeIntoCursor(value: UniffiDuration, c: Cursor): void {
      const ms = value.valueOf();
      const secsBigInt = BigInt(Math.trunc(ms)) / msPerSecBigInt;
      const remainingNanos = (ms % 1000) * nanosPerMs;
      c.writeU64(secsBigInt);
      c.writeU32(remainingNanos);
    }
    allocationSize(_value: UniffiDuration): number {
      return 12;
    }
  }
  return new FFIConverter();
})();

// We'll provide native js Date here; later on we'll need to put a Temporal based converter,
// and switch on from a config file.
export type UniffiTimestamp = Date;
export const FfiConverterTimestamp = (() => {
  const msPerSecBigInt = BigInt("1000");
  const nanosPerMs = 1e6;
  const msPerSec = 1e3;
  function safeDate(ms: number) {
    if (Math.abs(ms) > 8.64e15) {
      throw new UniffiInternalError.DateTimeOverflow();
    }
    return new Date(ms);
  }

  class FFIConverter extends AbstractFfiConverterByteArray<UniffiTimestamp> {
    readFromCursor(c: Cursor): UniffiTimestamp {
      const secsBigInt = c.readI64();
      const nanos = c.readU32();
      const ms = Number(secsBigInt * msPerSecBigInt);
      if (ms >= 0) {
        return safeDate(ms + nanos / nanosPerMs);
      } else {
        return safeDate(ms - nanos / nanosPerMs);
      }
    }
    writeIntoCursor(value: UniffiTimestamp, c: Cursor): void {
      const ms = value.valueOf();
      const secsBigInt = BigInt(Math.trunc(ms / msPerSec));
      const remainingNanos = Math.abs((ms % msPerSec) * nanosPerMs);
      c.writeI64(secsBigInt);
      c.writeU32(remainingNanos);
    }
    allocationSize(_value: UniffiTimestamp): number {
      return 12;
    }
  }
  return new FFIConverter();
})();

export class FfiConverterOptional<Item> extends AbstractFfiConverterByteArray<
  Item | undefined
> {
  constructor(private itemConverter: FfiConverter<any, Item>) {
    super();
  }
  readFromCursor(c: Cursor): Item | undefined {
    const tag = c.readU8();
    return tag === 0 ? undefined : this.itemConverter.readFromCursor(c);
  }
  writeIntoCursor(value: Item | undefined, c: Cursor): void {
    if (value === undefined) {
      c.writeU8(0);
      return;
    }
    c.writeU8(1);
    this.itemConverter.writeIntoCursor(value, c);
  }
  allocationSize(value: Item | undefined): number {
    return (
      1 + (value === undefined ? 0 : this.itemConverter.allocationSize(value))
    );
  }
}

export class FfiConverterArray<Item> extends AbstractFfiConverterByteArray<
  Array<Item>
> {
  constructor(private itemConverter: FfiConverter<any, Item>) {
    super();
  }
  readFromCursor(c: Cursor): Array<Item> {
    const size = c.readI32();
    const array = new Array<Item>(size);
    for (let i = 0; i < size; i++) {
      array[i] = this.itemConverter.readFromCursor(c);
    }
    return array;
  }
  writeIntoCursor(array: Array<Item>, c: Cursor): void {
    c.writeI32(array.length);
    for (const item of array) {
      this.itemConverter.writeIntoCursor(item, c);
    }
  }
  allocationSize(array: Array<Item>): number {
    let size = 4;
    for (const item of array) {
      size += this.itemConverter.allocationSize(item);
    }
    return size;
  }
}

export class FfiConverterMap<K, V> extends AbstractFfiConverterByteArray<
  Map<K, V>
> {
  constructor(
    private keyConverter: FfiConverter<any, K>,
    private valueConverter: FfiConverter<any, V>,
  ) {
    super();
  }
  readFromCursor(c: Cursor): Map<K, V> {
    const size = c.readI32();
    const map = new Map<K, V>();
    for (let i = 0; i < size; i++) {
      map.set(
        this.keyConverter.readFromCursor(c),
        this.valueConverter.readFromCursor(c),
      );
    }
    return map;
  }
  writeIntoCursor(map: Map<K, V>, c: Cursor): void {
    c.writeI32(map.size);
    for (const [k, v] of map.entries()) {
      this.keyConverter.writeIntoCursor(k, c);
      this.valueConverter.writeIntoCursor(v, c);
    }
  }
  allocationSize(map: Map<K, V>): number {
    let size = 4;
    for (const [k, v] of map.entries()) {
      size +=
        this.keyConverter.allocationSize(k) +
        this.valueConverter.allocationSize(v);
    }
    return size;
  }
}

export const FfiConverterArrayBuffer = (() => {
  class FFIConverter extends AbstractFfiConverterByteArray<ArrayBuffer> {
    readFromCursor(c: Cursor): ArrayBuffer {
      const length = c.readI32();
      return c.readArrayBuffer(length);
    }
    writeIntoCursor(value: ArrayBuffer, c: Cursor): void {
      c.writeI32(value.byteLength);
      c.writeBytes(new Uint8Array(value));
    }
    allocationSize(value: ArrayBuffer): number {
      return 4 + value.byteLength;
    }
  }
  return new FFIConverter();
})();

export const FfiConverterUint8Array = (() => {
  class FFIConverter extends AbstractFfiConverterByteArray<Uint8Array> {
    readFromCursor(c: Cursor): Uint8Array {
      const length = c.readI32();
      return c.readBytes(length);
    }
    writeIntoCursor(value: Uint8Array, c: Cursor): void {
      c.writeI32(value.byteLength);
      c.writeBytes(value);
    }
    allocationSize(value: Uint8Array): number {
      return 4 + value.byteLength;
    }
  }
  return new FFIConverter();
})();

type StringConverter = {
  // Single-string encoding. Each template picks an environment-appropriate
  // implementation: the JSI template uses a C++ helper (avoids the
  // TextEncoder allocation on the hot `lower()` path for large strings);
  // the WASM template uses `TextEncoder.encode`.
  stringToBytes: (s: string) => UniffiByteArray;
  bytesToString: (ab: UniffiByteArray) => string;
  stringByteLength: (s: string) => number;
  // Optional direct-buffer operations. When provided, `write` and `read` use
  // them to skip the intermediate Uint8Array produced by
  // `stringToBytes`/`bytesToString`. The `buf` argument is a RustBuffer; the
  // implementation encodes into / decodes from `buf.arrayBuffer` at the given
  // offset.
  // Mirrors `TextEncoder.encodeInto`: `written` is the byte count, `read` the
  // number of UTF-16 code units consumed. `read < s.length` means the string
  // did not fit — `written` cannot show that, because `encodeInto` stops
  // before a code point it lacks room to finish rather than filling the
  // buffer.
  writeStringIntoBuffer?: (
    s: string,
    buf: any,
    offset: number,
    capacity: number,
  ) => { read: number; written: number };
  readStringFromBuffer?: (buf: any, offset: number, length: number) => string;
};
export function uniffiCreateFfiConverterString(
  converter: StringConverter,
): FfiConverter<UniffiByteArray, string> {
  class FFIConverter implements FfiConverter<UniffiByteArray, string> {
    lift(value: UniffiByteArray): string {
      return converter.bytesToString(value);
    }
    lower(value: string, alloc: RustBufferAllocator): UniffiByteArray {
      // A top-level string is the whole buffer — no length prefix, unlike the
      // cursor path.
      //
      // `stringToBytes` transcodes into a native string and hands JS a buffer
      // that the boundary then copies again. Encoding straight into the
      // allocation removes both, the same way `writeIntoCursor` does for
      // strings inside a container. Empty strings keep the old path: a
      // zero-length allocation is not worth special-casing downstream.
      if (converter.writeStringIntoBuffer && value.length > 0) {
        // Size for ASCII first, where UTF-8 length equals UTF-16 length.
        // Reserving the 3-bytes-per-code-unit worst case up front would ask
        // the allocator for 3x the memory on every call, which costs more
        // than the occasional second pass — and the buffers are Rust-owned,
        // so on the JSI player they are only released when the view is
        // collected.
        //
        // The extra byte makes truncation detectable: a string that fits
        // leaves at least one byte unwritten, so `written === capacity` can
        // only mean `encodeInto` ran out of room.
        let capacity = value.length;
        let view = alloc(capacity);
        let result = converter.writeStringIntoBuffer(
          value,
          { arrayBuffer: view.buffer } as any,
          view.byteOffset,
          capacity,
        );
        if (result.read < value.length) {
          // Some of the string didn't fit, so it holds at least one non-ASCII
          // code point. Redo at the worst case, which always suffices.
          capacity = 3 * value.length;
          view = alloc(capacity);
          result = converter.writeStringIntoBuffer(
            value,
            { arrayBuffer: view.buffer } as any,
            view.byteOffset,
            capacity,
          );
        }
        const written = result.written;
        if (written === capacity) {
          return view;
        }
        const exact = view.subarray(0, written);
        (exact as any).__ubrnRustCapacity = capacity;
        return exact;
      }
      return converter.stringToBytes(value);
    }
    readFromCursor(c: Cursor): string {
      const length = c.readI32();
      if (converter.readStringFromBuffer) {
        // Read directly from the cursor's backing ArrayBuffer — zero copy.
        // The helper takes an object with an `arrayBuffer` property plus an
        // absolute offset and length; for Cursor that's `u8.buffer` and
        // `start + pos`.
        const buf: any = { arrayBuffer: (c as any).u8.buffer };
        const offset = (c as any).start + (c as any).pos;
        (c as any).pos += length;
        return converter.readStringFromBuffer(buf, offset, length);
      }
      const bytes = c.readBytes(length);
      return converter.bytesToString(bytes);
    }
    writeIntoCursor(value: string, c: Cursor): void {
      if (converter.writeStringIntoBuffer) {
        // Encode the string into the buffer at the data offset (after the
        // i32 length prefix), then backfill the length prefix with the
        // actual bytes written. Skips one `stringByteLength` measurement
        // call per string compared to the writeBytes path below.
        const lengthPos = (c as any).pos;
        // Reserve 4 bytes for the length prefix.
        (c as any).pos += 4;
        const dataOffset = (c as any).start + (c as any).pos;
        // Synthetic buf with the cursor's underlying ArrayBuffer; helpers
        // use `buf.arrayBuffer` to construct a Uint8Array view.
        const buf: any = { arrayBuffer: (c as any).u8.buffer };
        // `allocationSize` reserved the worst case for this string, so the
        // encode always fits and `read` needs no checking here.
        const { written: bytesWritten } = converter.writeStringIntoBuffer(
          value,
          buf,
          dataOffset,
          3 * value.length,
        );
        // Backfill the length prefix (big-endian i32, matches Cursor.writeI32).
        (c as any).dv.setInt32(lengthPos, bytesWritten);
        (c as any).pos += bytesWritten;
        return;
      }
      const bytes = converter.stringToBytes(value);
      c.writeI32(bytes.byteLength);
      c.writeBytes(bytes);
    }
    allocationSize(value: string): number {
      if (converter.writeStringIntoBuffer) {
        // Upper bound rather than an exact measurement. `stringByteLength`
        // transcodes the whole string and keeps only its length, so sizing
        // exactly costs a second full UTF-8 encode of every string — the
        // dominant cost when lowering arrays of them.
        //
        // UTF-8 needs at most 3 bytes per UTF-16 code unit: a BMP character
        // is one unit and at most 3 bytes, and a surrogate pair is two units
        // and 4 bytes. `writeIntoCursor` backfills the true length and
        // `lower()` shrinks the view to it.
        return 4 + 3 * value.length;
      }
      return 4 + converter.stringByteLength(value);
    }
  }
  return new FFIConverter();
}
