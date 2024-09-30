/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  identityArrayBuffer,
  identityArrayBufferForcedRead,
  wellKnownArrayBuffer,
} from "../../generated/uniffi_coverall2";
import { test } from "@/asserts";
import { console } from "@/hermes";

test("well known array buffer returned", (t) => {
  const wellKnown = wellKnownArrayBuffer();
  t.assertEqual(0, wellKnown.byteLength);
});

test("array buffer equals", (t) => {
  t.assertEqual(arrayBuffer(16).byteLength, 16);
  t.assertEqual(arrayBuffer(16), arrayBuffer(16), undefined, abEquals);

  const mutated = new Uint32Array(arrayBuffer(32), 0).reverse().buffer;
  t.assertNotEqual(mutated, arrayBuffer(32), undefined, abEquals);
});

test("array buffer roundtrip using lift/lower", (t) => {
  function rt(ab: ArrayBuffer) {
    t.assertEqual(ab, identityArrayBuffer(ab), undefined, abEquals);
  }
  for (let i = 0; i < 64; i++) {
    rt(arrayBuffer(i));
  }
});

test("array buffer roundtrip using read/write", (t) => {
  function rt(ab: ArrayBuffer) {
    t.assertEqual(ab, identityArrayBufferForcedRead(ab)!, undefined, abEquals);
  }
  for (let i = 0; i < 64; i++) {
    rt(arrayBuffer(i));
  }
});

test("ArrayBuffer roundtrip of different sizes", (t) => {
  function rt(ab: ArrayBuffer) {
    t.assertNotNull(identityArrayBuffer(ab)!);
  }
  // 1 kB = 1<<10
  // 1 MB = 1<<20
  // 16 MB = 1<<24
  for (let i = 0; i < 26; i++) {
    const byteLength = 1 << i;
    const buffer = new ArrayBuffer(byteLength);
    const start = Date.now();
    rt(buffer);
    const end = Date.now();
    console.log(
      `ArrayBuffer roundtrip: ${bytes(byteLength)} in ${end - start} ms`,
    );
  }
});

function bytes(n: number): string {
  if (n === 0) {
    return "0 bytes";
  }
  if (n < 1 << 10) {
    return `${n} bytes`;
  }
  if (n < 1 << 20) {
    return `${n / (1 << 10)} kB`;
  }
  if (n < 1 << 30) {
    return `${n / (1 << 20)} MB`;
  }
  return `${n / (1 << 30)} GB`;
}

test("array buffer roundtrip with ArrayBufferView", (t) => {
  function rt(ab: ArrayBuffer) {
    t.assertEqual(
      ab,
      identityArrayBuffer(new Uint32Array(ab)),
      undefined,
      abEquals,
    );
  }
  for (let i = 0; i < 64; i += 4) {
    rt(arrayBuffer(i));
  }
});

test("array buffer roundtrip with ArrayBufferView of different sizes", (t) => {
  function rt(ta: ArrayBuffer, slice: ArrayBuffer) {
    t.assertEqual(slice, identityArrayBuffer(ta), undefined, abEquals);
  }
  const base = arrayBuffer(64);
  for (const TypedArray of [
    Uint8Array,
    Uint16Array,
    Uint32Array,
    BigUint64Array,
    Int8Array,
    Int16Array,
    Int32Array,
    BigInt64Array,
    Float32Array,
    Float64Array,
  ]) {
    const width = TypedArray.BYTES_PER_ELEMENT;
    for (let length = width; length < base.byteLength; length += width) {
      for (
        let offset = 0;
        offset + length < base.byteLength;
        offset += length
      ) {
        const typedArray = new TypedArray(base, offset, length / width);
        const slice = base.slice(offset, offset + length);
        rt(typedArray, slice);
      }
    }
  }
});

function abEquals(a: ArrayBuffer, b: ArrayBuffer): boolean {
  if (a.byteLength !== b.byteLength) {
    return false;
  }

  const len = a.byteLength;
  const aArray = new Uint8Array(a);
  const bArray = new Uint8Array(b);

  for (let i = 0; i < len; i++) {
    if (aArray.at(i) !== bArray.at(i)) {
      return false;
    }
  }

  return true;
}

function arrayBuffer(numBytes: number): ArrayBuffer {
  const array = Uint8Array.from({ length: numBytes }, (_v, i) => i % 255);
  return array.buffer;
}
