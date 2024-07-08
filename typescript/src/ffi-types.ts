/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

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

  static fromArrayBuffer(buf: ArrayBuffer) {
    return new RustBuffer(buf);
  }

  get length(): number {
    return this.arrayBuffer.byteLength;
  }

  read<T>(numBytes: number, reader: (buffer: ArrayBuffer) => T | undefined): T {
    const start = this.readOffset;
    const end = this.checkOverflow(start, numBytes);
    const value = reader(this.arrayBuffer.slice(start, end));
    this.readOffset = end;
    return value as T;
  }

  write(numBytes: number, writer: () => ArrayBuffer) {
    const start = this.writeOffset;
    const end = this.checkOverflow(start, numBytes);

    const slice = writer();
    if (numBytes !== slice.byteLength) {
      throw new UniffiInternalError.IncompleteData();
    }

    // copy slice into the original arraybuffer.
    const src = new Uint8Array(slice);
    const dest = new Uint8Array(this.arrayBuffer, start);
    dest.set(src);

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

export type ForeignBytes = ArrayBuffer;
