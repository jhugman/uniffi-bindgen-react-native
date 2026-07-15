/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

/**
 * Positional reader/writer over an `ArrayBuffer` slice with monomorphic typed
 * accessors. One `DataView` and one `Uint8Array` are built at construction and
 * reused for every primitive operation — no per-read DataView allocation, no
 * function-pointer dispatch.
 *
 * Big-endian on the wire (matches the UniFFI RustBuffer wire format).
 */
export class Cursor {
  private readonly dv: DataView;
  private readonly u8: Uint8Array;
  private readonly start: number;
  private readonly end: number;
  private pos: number;

  constructor(buffer: ArrayBuffer, offset: number, length: number) {
    this.dv = new DataView(buffer, offset, length);
    this.u8 = new Uint8Array(buffer, offset, length);
    this.start = offset;
    this.end = offset + length;
    this.pos = 0;
  }

  static fromUint8Array(view: Uint8Array): Cursor {
    return new Cursor(
      view.buffer as ArrayBuffer,
      view.byteOffset,
      view.byteLength,
    );
  }

  remaining(): number {
    return this.end - this.start - this.pos;
  }

  skip(n: number): void {
    this.boundsCheck(n);
    this.pos += n;
  }

  readU8(): number {
    this.boundsCheck(1);
    const v = this.dv.getUint8(this.pos);
    this.pos += 1;
    return v;
  }
  readI8(): number {
    this.boundsCheck(1);
    const v = this.dv.getInt8(this.pos);
    this.pos += 1;
    return v;
  }
  readU16(): number {
    this.boundsCheck(2);
    const v = this.dv.getUint16(this.pos);
    this.pos += 2;
    return v;
  }
  readI16(): number {
    this.boundsCheck(2);
    const v = this.dv.getInt16(this.pos);
    this.pos += 2;
    return v;
  }
  readU32(): number {
    this.boundsCheck(4);
    const v = this.dv.getUint32(this.pos);
    this.pos += 4;
    return v;
  }
  readI32(): number {
    this.boundsCheck(4);
    const v = this.dv.getInt32(this.pos);
    this.pos += 4;
    return v;
  }
  readU64(): bigint {
    this.boundsCheck(8);
    const v = this.dv.getBigUint64(this.pos);
    this.pos += 8;
    return v;
  }
  readI64(): bigint {
    this.boundsCheck(8);
    const v = this.dv.getBigInt64(this.pos);
    this.pos += 8;
    return v;
  }
  readF32(): number {
    this.boundsCheck(4);
    const v = this.dv.getFloat32(this.pos);
    this.pos += 4;
    return v;
  }
  readF64(): number {
    this.boundsCheck(8);
    const v = this.dv.getFloat64(this.pos);
    this.pos += 8;
    return v;
  }
  readBool(): boolean {
    return this.readU8() !== 0;
  }

  writeU8(v: number): void {
    this.boundsCheck(1);
    this.dv.setUint8(this.pos, v);
    this.pos += 1;
  }
  writeI8(v: number): void {
    this.boundsCheck(1);
    this.dv.setInt8(this.pos, v);
    this.pos += 1;
  }
  writeU16(v: number): void {
    this.boundsCheck(2);
    this.dv.setUint16(this.pos, v);
    this.pos += 2;
  }
  writeI16(v: number): void {
    this.boundsCheck(2);
    this.dv.setInt16(this.pos, v);
    this.pos += 2;
  }
  writeU32(v: number): void {
    this.boundsCheck(4);
    this.dv.setUint32(this.pos, v);
    this.pos += 4;
  }
  writeI32(v: number): void {
    this.boundsCheck(4);
    this.dv.setInt32(this.pos, v);
    this.pos += 4;
  }
  writeU64(v: bigint): void {
    this.boundsCheck(8);
    this.dv.setBigUint64(this.pos, v);
    this.pos += 8;
  }
  writeI64(v: bigint): void {
    this.boundsCheck(8);
    this.dv.setBigInt64(this.pos, v);
    this.pos += 8;
  }
  writeF32(v: number): void {
    this.boundsCheck(4);
    this.dv.setFloat32(this.pos, v);
    this.pos += 4;
  }
  writeF64(v: number): void {
    this.boundsCheck(8);
    this.dv.setFloat64(this.pos, v);
    this.pos += 8;
  }
  writeBool(v: boolean): void {
    this.writeU8(v ? 1 : 0);
  }

  readBytes(len: number): Uint8Array {
    this.boundsCheck(len);
    // start is an absolute offset into the underlying buffer; this matters
    // when the Cursor wraps a sub-window of a larger buffer (e.g. via
    // fromUint8Array over a Node.js Buffer pool).
    const start = this.start + this.pos;
    const out = new Uint8Array(this.u8.buffer.slice(start, start + len));
    this.pos += len;
    return out;
  }

  readArrayBuffer(len: number): ArrayBuffer {
    this.boundsCheck(len);
    const start = this.start + this.pos;
    const out = (this.u8.buffer as ArrayBuffer).slice(start, start + len);
    this.pos += len;
    return out;
  }

  writeBytes(src: Uint8Array): void {
    this.boundsCheck(src.byteLength);
    this.u8.set(src, this.pos);
    this.pos += src.byteLength;
  }

  private boundsCheck(n: number): void {
    if (this.pos + n > this.end - this.start) {
      throw new UniffiInternalError.BufferOverflow();
    }
  }
}
