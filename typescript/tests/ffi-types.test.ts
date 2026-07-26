/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { RustBuffer } from "../src/ffi-types";
import { test, xtest } from "../testing/asserts";

// `WebAssembly` is not in scope in Hermes (Jsi runtime); access it through
// `globalThis` so the test source compiles on every flavor and we can skip
// the body where the API is missing.
const WA: any = (globalThis as any).WebAssembly;
const itTest = WA !== undefined ? test : xtest;

itTest(
  "RustBuffer.fromWasmMemory reads from a wasm-memory-backed slice",
  (t) => {
    const wm = new WA.Memory({ initial: 1 });
    const view = new Uint8Array(wm.buffer);
    view[100] = 0xab;
    view[101] = 0xcd;

    const rb = RustBuffer.fromWasmMemory(wm.buffer, 100, 2);
    const readBack = rb.readByteArray(2);
    t.assertEqual(Array.from(readBack), [0xab, 0xcd]);
  },
);

itTest("RustBuffer.fromWasmMemory writes through to wasm memory", (t) => {
  const wm = new WA.Memory({ initial: 1 });
  const rb = RustBuffer.fromWasmMemory(wm.buffer, 200, 3);
  rb.writeByteArray(new Uint8Array([0x01, 0x02, 0x03]));

  const view = new Uint8Array(wm.buffer);
  t.assertEqual(view[200], 0x01);
  t.assertEqual(view[201], 0x02);
  t.assertEqual(view[202], 0x03);
});

test("RustBuffer.fromUint8Array honours non-zero byteOffset", (t) => {
  const backing = new ArrayBuffer(64);
  const seed = new Uint8Array(backing);
  seed[40] = 0x11;
  seed[41] = 0x22;
  seed[42] = 0x33;

  const view = new Uint8Array(backing, 40, 3);
  const rb = RustBuffer.fromUint8Array(view);

  t.assertEqual(Array.from(rb.readByteArray(3)), [0x11, 0x22, 0x33]);
});

test("RustBuffer.fromUint8Array writes through to backing storage", (t) => {
  const backing = new ArrayBuffer(64);
  const view = new Uint8Array(backing, 16, 4);
  const rb = RustBuffer.fromUint8Array(view);
  rb.writeByteArray(new Uint8Array([0x01, 0x02, 0x03, 0x04]));

  const seed = new Uint8Array(backing);
  t.assertEqual(seed[16], 0x01);
  t.assertEqual(seed[17], 0x02);
  t.assertEqual(seed[18], 0x03);
  t.assertEqual(seed[19], 0x04);
});
