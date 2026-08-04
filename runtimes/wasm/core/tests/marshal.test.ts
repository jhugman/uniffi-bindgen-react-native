/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { Memory } from "../src/memory.js";
import {
  readRustBuffer,
  writeRustBuffer,
  readRustCallStatus,
  writeRustCallStatusZero,
  readForeignBytes,
  writeForeignBytes,
  RUST_BUFFER_SIZE,
  RUST_CALL_STATUS_SIZE,
  FOREIGN_BYTES_SIZE,
  liftRustBufferOverMemory,
} from "../src/marshal.js";
import { compileStructLayout } from "../src/marshal.js";
import { FfiType } from "../src/ffi-type.js";
// Same module instance `marshal.ts` uses, so `instanceof` holds.
import { RustBuffer } from "@ubjs/core";

test("RustBuffer round-trip (capacity, len, dataPtr)", () => {
  const m = new Memory(new WebAssembly.Memory({ initial: 1 }));
  const rb = { capacity: 1234n, len: 56n, dataPtr: 0xc0ffee };
  writeRustBuffer(m, 0x100, rb);
  assert.deepStrictEqual(readRustBuffer(m, 0x100), rb);
  assert.strictEqual(RUST_BUFFER_SIZE, 24);
});

test("writeRustCallStatusZero writes 32 zero bytes", () => {
  const m = new Memory(new WebAssembly.Memory({ initial: 1 }));
  // Prefill nonzero
  for (let i = 0; i < 32; i++) m.writeU8(0x200 + i, 0xff);
  writeRustCallStatusZero(m, 0x200);
  for (let i = 0; i < 32; i++) assert.strictEqual(m.readU8(0x200 + i), 0);
  assert.strictEqual(RUST_CALL_STATUS_SIZE, 32);
});

test("readRustCallStatus picks up code + error_buf", () => {
  const m = new Memory(new WebAssembly.Memory({ initial: 1 }));
  writeRustCallStatusZero(m, 0x200);
  m.writeU8(0x200, 1); // code = 1 (error)
  writeRustBuffer(m, 0x200 + 8, { capacity: 10n, len: 5n, dataPtr: 0x500 });
  const s = readRustCallStatus(m, 0x200);
  assert.strictEqual(s.code, 1);
  assert.deepStrictEqual(s.errorBuf, {
    capacity: 10n,
    len: 5n,
    dataPtr: 0x500,
  });
});

test("ForeignBytes round-trip (i32 len, ptr)", () => {
  const m = new Memory(new WebAssembly.Memory({ initial: 1 }));
  writeForeignBytes(m, 0x300, { len: 7, dataPtr: 0xabcd });
  assert.deepStrictEqual(readForeignBytes(m, 0x300), {
    len: 7,
    dataPtr: 0xabcd,
  });
  assert.strictEqual(FOREIGN_BYTES_SIZE, 8);
});

test("liftRustBufferOverMemory aliases wasm memory directly", () => {
  const wm = new WebAssembly.Memory({ initial: 1 });
  const view = new Uint8Array(wm.buffer);
  view.set([1, 2, 3, 4], 200);
  const m = new Memory(wm);
  const rb = liftRustBufferOverMemory(m, {
    capacity: 4n,
    len: 4n,
    dataPtr: 200,
  });
  assert.ok(rb instanceof RustBuffer);
  assert.deepStrictEqual(Array.from(rb.readByteArray(4)), [1, 2, 3, 4]);
});

test("compileStructLayout produces a working pair for {u32, u8, RustBuffer}", () => {
  const m = new Memory(new WebAssembly.Memory({ initial: 1 }));
  const fields = [
    { name: "a", type: FfiType.UInt32 },
    { name: "b", type: FfiType.UInt8 },
    { name: "rb", type: FfiType.RustBuffer },
  ];
  const { read, write, size } = compileStructLayout(fields);
  assert.strictEqual(size, 4 + 1 + 3 /*pad*/ + 24);

  const value = {
    a: 0x11223344,
    b: 0x55,
    rb: { capacity: 100n, len: 50n, dataPtr: 0x666 },
  };
  write(m, 0x400, value);
  assert.deepStrictEqual(read(m, 0x400), value);
});
