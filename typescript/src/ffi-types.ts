import { UniffiInternalError } from "./errors";

export class RustBuffer {
  private readOffset: number = 0;
  private writeOffset: number = 0;
  public arrayBuffer: ArrayBuffer;

  private constructor(arrayBuffer: ArrayBuffer) {
    this.arrayBuffer = arrayBuffer;
  }

  static withCapacity(capacity: number): RustBuffer {
    const buf = new ArrayBuffer(capacity);
    return new RustBuffer(buf);
  }

  static fromArrayBuffer(buf: ArrayBuffer) {
    return new RustBuffer(buf);
  }

  get length(): number {
    return this.arrayBuffer.byteLength;
  }

  read<T>(
    numBytes: number,
    reader: (buffer: ArrayBuffer, offset: number) => T | undefined,
  ): T {
    const value = reader(this.arrayBuffer, this.readOffset);
    this.readOffset += numBytes;
    if (value === undefined) {
      throw new UniffiInternalError.BufferOverflow();
    }
    return value as T;
  }

  write<T>(
    numBytes: number,
    writer: (buffer: ArrayBuffer, offset: number) => void,
  ) {
    writer(this.arrayBuffer, this.writeOffset);
    this.writeOffset += numBytes;
  }

  deallocate() {
    // call into Rust, perhaps.
  }
}

export type ForeignBytes = ArrayBuffer;
