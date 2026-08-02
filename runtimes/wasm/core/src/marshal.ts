/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { Memory } from "./memory.js";
import type { FfiTypeDesc } from "./ffi-type.js";
import { RustBuffer } from "@ubjs/core";

export const RUST_BUFFER_SIZE = 24; // capacity:u64, len:u64, dataPtr:u32 (+pad)
export const RUST_CALL_STATUS_SIZE = 32; // code:i8 + pad + RustBuffer
export const FOREIGN_BYTES_SIZE = 8; // len:i32, dataPtr:u32

const RB_CAPACITY_OFF = 0;
const RB_LEN_OFF = 8;
const RB_DATAPTR_OFF = 16;

export interface RustBufferLike {
  capacity: bigint;
  len: bigint;
  dataPtr: number;
}

export function readRustBuffer(m: Memory, ptr: number): RustBufferLike {
  return {
    capacity: m.readU64(ptr + RB_CAPACITY_OFF),
    len: m.readU64(ptr + RB_LEN_OFF),
    dataPtr: m.readU32(ptr + RB_DATAPTR_OFF),
  };
}

export function writeRustBuffer(m: Memory, ptr: number, rb: RustBufferLike) {
  m.writeU64(ptr + RB_CAPACITY_OFF, rb.capacity);
  m.writeU64(ptr + RB_LEN_OFF, rb.len);
  m.writeU32(ptr + RB_DATAPTR_OFF, rb.dataPtr);
}

/**
 * Write a 24-byte `RustBuffer` struct at `ptr` (`capacity = len = bytes
 * .byteLength`, `dataPtr` pointing at the payload bytes inside wasm memory).
 *
 * Two payload paths:
 *
 * 1. **Wasm-aliased view (`lower(value, rustbuffer_alloc)`):** the converter
 *    has already written its bytes directly into a wasm allocation obtained
 *    from `rustbuffer_alloc(n)` — `bytes.buffer` is the wasm linear memory
 *    and `bytes.byteOffset` is the `dataPtr`. We just forward
 *    `(byteOffset, byteLength)` into the struct.
 *
 * 2. **JS-owned `Uint8Array`:** allocate a fresh wasm buffer of the right
 *    size and copy bytes in.
 *
 * Used on every wasm2 lower path that hands a `Uint8Array` to Rust through
 * a `RustBuffer` slot — top-level args, callback args/returns,
 * `RustCallStatus.errorBuf`, and `RustBuffer` fields inside structs.
 * Ownership of the payload bytes transfers to Rust, so the caller does
 * not free `dataPtr`.
 *
 * Throws if `bytes` is a detached view. A detached view is unrecoverable
 * (we cannot read its contents to copy them, and the source buffer is no
 * longer valid wasm memory either). It indicates a contract violation —
 * a wasm-aliased view was held across an operation that grew wasm memory.
 */
export function writeRustBufferPayload(
  m: Memory,
  alloc: (size: number, align: number) => number,
  ptr: number,
  bytes: Uint8Array,
): void {
  const len = bytes.byteLength;
  let dataPtr = 0;
  if (len > 0) {
    if (bytes.buffer === m.buffer()) {
      // Already aliased to wasm memory — no realloc, no copy.
      dataPtr = bytes.byteOffset;
    } else if (bytes.buffer.byteLength === 0) {
      // The source view's ArrayBuffer was detached, almost certainly because
      // wasm memory grew between `rustbuffer_alloc(n)` and this call. We
      // cannot recover: the bytes are unreadable.
      throw new Error(
        "writeRustBufferPayload: source view is detached — wasm memory grew " +
          "between rustbuffer_alloc(n) and the FFI call. The view must not " +
          "be held across any operation that can grow wasm memory.",
      );
    } else {
      // JS-owned `Uint8Array` — allocate wasm memory and copy.
      dataPtr = alloc(len, 1);
      m.writeBytes(dataPtr, bytes);
    }
  }
  const blen = BigInt(len);
  writeRustBuffer(m, ptr, { capacity: blen, len: blen, dataPtr });
}

const RCS_CODE_OFF = 0;
export const RCS_ERROR_BUF_OFF = 8;

export interface RustCallStatusLike {
  code: number;
  errorBuf: RustBufferLike;
}

export function readRustCallStatus(m: Memory, ptr: number): RustCallStatusLike {
  return {
    code: m.readU8(ptr + RCS_CODE_OFF),
    errorBuf: readRustBuffer(m, ptr + RCS_ERROR_BUF_OFF),
  };
}

export function writeRustCallStatusZero(m: Memory, ptr: number) {
  // Status is exactly 32 bytes; write zeros via two u64 + one u64 + alignment.
  for (let i = 0; i < RUST_CALL_STATUS_SIZE; i += 8) {
    m.writeU64(ptr + i, 0n);
  }
}

const FB_LEN_OFF = 0;
const FB_DATAPTR_OFF = 4;

export interface ForeignBytesLike {
  len: number;
  dataPtr: number;
}

export function readForeignBytes(m: Memory, ptr: number): ForeignBytesLike {
  return {
    len: m.readI32(ptr + FB_LEN_OFF),
    dataPtr: m.readU32(ptr + FB_DATAPTR_OFF),
  };
}

export function writeForeignBytes(
  m: Memory,
  ptr: number,
  fb: ForeignBytesLike,
) {
  m.writeI32(ptr + FB_LEN_OFF, fb.len);
  m.writeU32(ptr + FB_DATAPTR_OFF, fb.dataPtr);
}

export interface FieldDesc {
  name: string;
  type: FfiTypeDesc;
}

export interface CompiledField {
  name: string;
  offset: number;
  type: FfiTypeDesc;
  shape: ScalarShape;
}

export interface StructLayout {
  size: number;
  // Per-field offset/shape/type, in declaration order. The dispatcher walks
  // these directly when an arg of type `Reference(Struct(name))` needs custom
  // per-field handling — e.g. installing a JS function as a wasm callback for
  // `Callback`-typed fields in vtables.
  fields: CompiledField[];
  read: (m: Memory, ptr: number) => Record<string, unknown>;
  write: (m: Memory, ptr: number, value: Record<string, unknown>) => void;
}

export interface ScalarShape {
  size: number;
  align: number;
  read: (m: Memory, p: number) => unknown;
  write: (m: Memory, p: number, v: any) => void;
}

function shapeFor(type: FfiTypeDesc): ScalarShape {
  switch (type.tag) {
    case "UInt8":
    case "Int8":
      return {
        size: 1,
        align: 1,
        read: (m, p) => m.readU8(p),
        write: (m, p, v) => m.writeU8(p, v),
      };
    case "UInt32":
    case "Int32":
    case "VoidPointer":
    case "Callback":
    case "Reference":
    case "MutReference":
      return {
        size: 4,
        align: 4,
        read: (m, p) => m.readU32(p),
        write: (m, p, v) => m.writeU32(p, v),
      };
    case "UInt64":
    case "Int64":
    // UniFFI handles are u64 (registry slots); reading/writing them as 32-bit
    // would silently drop the high half — see e.g.
    // `ForeignFutureDroppedCallbackStruct` which has a `handle: Handle` field.
    case "Handle":
      return {
        size: 8,
        align: 8,
        read: (m, p) => m.readU64(p),
        write: (m, p, v) => m.writeU64(p, v),
      };
    case "Float32":
      return {
        size: 4,
        align: 4,
        read: (m, p) => m.readF32(p),
        write: (m, p, v) => m.writeF32(p, v),
      };
    case "Float64":
      return {
        size: 8,
        align: 8,
        read: (m, p) => m.readF64(p),
        write: (m, p, v) => m.writeF64(p, v),
      };
    case "RustBuffer":
      return {
        size: RUST_BUFFER_SIZE,
        align: 8,
        read: (m, p) => readRustBuffer(m, p),
        write: (m, p, v) => writeRustBuffer(m, p, v),
      };
    case "ForeignBytes":
      return {
        size: FOREIGN_BYTES_SIZE,
        align: 4,
        read: (m, p) => readForeignBytes(m, p),
        write: (m, p, v) => writeForeignBytes(m, p, v),
      };
    case "RustCallStatus":
      return {
        size: RUST_CALL_STATUS_SIZE,
        align: 8,
        read: (m, p) => readRustCallStatus(m, p),
        write: () => {
          throw new Error("RustCallStatus is not directly writable");
        },
      };
    default:
      throw new Error(
        `shapeFor: nested type ${type.tag} not yet supported in struct fields`,
      );
  }
}

const align = (off: number, a: number) => (off + (a - 1)) & ~(a - 1);

export function compileStructLayout(fields: FieldDesc[]): StructLayout {
  const compiled: CompiledField[] = [];
  let off = 0;
  let maxAlign = 1;
  for (const f of fields) {
    const shape = shapeFor(f.type);
    off = align(off, shape.align);
    compiled.push({ name: f.name, offset: off, type: f.type, shape });
    off += shape.size;
    if (shape.align > maxAlign) maxAlign = shape.align;
  }
  const size = align(off, maxAlign);

  return {
    size,
    fields: compiled,
    read(m, ptr) {
      const out: Record<string, unknown> = {};
      for (const c of compiled) out[c.name] = c.shape.read(m, ptr + c.offset);
      return out;
    },
    write(m, ptr, value) {
      for (const c of compiled)
        c.shape.write(m, ptr + c.offset, (value as any)[c.name]);
    },
  };
}

/**
 * Lift a `RustBufferLike` view into a `RustBuffer` that aliases the wasm
 * `Memory.buffer()` directly — no intermediate copy. Use this on the hot
 * path for record-shaped types so lift/lower can read/write straight out
 * of wasm memory. The returned `RustBuffer` is only valid until the next
 * memory growth or boundary crossing that may detach the buffer.
 */
export function liftRustBufferOverMemory(
  m: Memory,
  rb: RustBufferLike,
): RustBuffer {
  return RustBuffer.fromWasmMemory(m.buffer(), rb.dataPtr, Number(rb.len));
}
