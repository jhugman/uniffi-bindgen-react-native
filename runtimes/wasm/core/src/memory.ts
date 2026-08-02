/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export class Memory {
  private memory: WebAssembly.Memory;
  private cachedDv: DataView;
  private cachedU8: Uint8Array;

  constructor(memory: WebAssembly.Memory) {
    this.memory = memory;
    this.cachedDv = new DataView(memory.buffer);
    this.cachedU8 = new Uint8Array(memory.buffer);
  }

  /** Always-current DataView. Cheap if memory has not grown. */
  dv(): DataView {
    if (this.cachedDv.buffer !== this.memory.buffer) {
      this.cachedDv = new DataView(this.memory.buffer);
    }
    return this.cachedDv;
  }

  /** Always-current Uint8Array. Cheap if memory has not grown. */
  view(): Uint8Array {
    if (this.cachedU8.buffer !== this.memory.buffer) {
      this.cachedU8 = new Uint8Array(this.memory.buffer);
    }
    return this.cachedU8;
  }

  buffer(): ArrayBuffer {
    return this.memory.buffer;
  }

  readU8(ptr: number): number {
    return this.dv().getUint8(ptr);
  }
  readU32(ptr: number): number {
    return this.dv().getUint32(ptr, true);
  }
  readI32(ptr: number): number {
    return this.dv().getInt32(ptr, true);
  }
  readU64(ptr: number): bigint {
    return this.dv().getBigUint64(ptr, true);
  }
  readF32(ptr: number): number {
    return this.dv().getFloat32(ptr, true);
  }
  readF64(ptr: number): number {
    return this.dv().getFloat64(ptr, true);
  }

  writeU8(ptr: number, v: number) {
    this.dv().setUint8(ptr, v);
  }
  writeU32(ptr: number, v: number) {
    this.dv().setUint32(ptr, v, true);
  }
  writeI32(ptr: number, v: number) {
    this.dv().setInt32(ptr, v, true);
  }
  writeU64(ptr: number, v: bigint) {
    this.dv().setBigUint64(ptr, v, true);
  }
  writeF32(ptr: number, v: number) {
    this.dv().setFloat32(ptr, v, true);
  }
  writeF64(ptr: number, v: number) {
    this.dv().setFloat64(ptr, v, true);
  }

  /** Returns a fresh `Uint8Array` copy of `[ptr, ptr+len)`. */
  readBytes(ptr: number, len: number): Uint8Array {
    return this.view().slice(ptr, ptr + len);
  }

  writeBytes(ptr: number, src: Uint8Array) {
    this.view().set(src, ptr);
  }
}
