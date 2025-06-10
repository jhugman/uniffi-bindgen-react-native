/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

export type UniffiByteArray = Uint8Array;

export class RustBuffer {
  private readOffset: number = 0;
  private writeOffset: number = 0;
  private capacity: number;
  public arrayBuffer: ArrayBuffer;

  private constructor(arrayBuffer: ArrayBuffer) {
    this.arrayBuffer = arrayBuffer;
    this.capacity = arrayBuffer.byteLength;
  }

  static withCapacity(capacity: number): RustBuffer {
    const buf = new ArrayBuffer(capacity);
    return new RustBuffer(buf);
  }

  static empty(): RustBuffer {
    return this.withCapacity(0);
  }

  static fromArrayBuffer(buf: ArrayBuffer): RustBuffer {
    return new RustBuffer(buf);
  }

  static fromByteArray(buf: UniffiByteArray): RustBuffer {
    return new RustBuffer(buf.buffer as ArrayBuffer);
  }

  get length(): number {
    return this.arrayBuffer.byteLength;
  }

  get byteArray(): UniffiByteArray {
    return new Uint8Array(this.arrayBuffer);
  }

  readArrayBuffer(numBytes: number): ArrayBuffer {
    const start = this.readOffset;
    const end = this.checkOverflow(start, numBytes);
    const value = this.arrayBuffer.slice(start, end);
    this.readOffset = end;
    return value;
  }

  readByteArray(numBytes: number): UniffiByteArray {
    const start = this.readOffset;
    const end = this.checkOverflow(start, numBytes);
    const value = new Uint8Array(this.arrayBuffer, start, numBytes);
    this.readOffset = end;
    return value;
  }

  writeArrayBuffer(buffer: ArrayBufferLike) {
    const start = this.writeOffset;
    const end = this.checkOverflow(start, buffer.byteLength);

    const src = new Uint8Array(buffer);
    const dest = new Uint8Array(this.arrayBuffer, start);
    dest.set(src);

    this.writeOffset = end;
  }

  writeByteArray(src: UniffiByteArray) {
    const start = this.writeOffset;
    const end = this.checkOverflow(start, src.byteLength);
    const dest = new Uint8Array(this.arrayBuffer, start);
    dest.set(src);

    this.writeOffset = end;
  }

  readWithView<T>(numBytes: number, reader: (view: DataView) => T): T {
    const start = this.readOffset;
    const end = this.checkOverflow(start, numBytes);
    const view = new DataView(this.arrayBuffer, start, numBytes);
    const value = reader(view);
    this.readOffset = end;
    return value as T;
  }

  writeWithView(numBytes: number, writer: (view: DataView) => void) {
    const start = this.writeOffset;
    const end = this.checkOverflow(start, numBytes);
    const view = new DataView(this.arrayBuffer, start, numBytes);
    writer(view);
    this.writeOffset = end;
  }

  protected checkOverflow(start: number, numBytes: number): number {
    const end = start + numBytes;
    if (this.capacity < end) {
      throw new UniffiInternalError.BufferOverflow();
    }
    return end;
  }
}
