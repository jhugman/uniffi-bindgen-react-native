/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-coverall2 -- jsi
//   cargo test -p uniffi-fixture-coverall2 -- wasm

import {
  identityArrayBuffer,
  identityArrayBufferForcedRead,
  identityNestedOptional,
  matchNestedOptional,
  wellKnownArrayBuffer,
} from "@/generated/uniffi_coverall2";
import { test, xtest } from "@/asserts";
import "@/polyfills";

test("well known array buffer returned", (t) => {
  const wellKnown = wellKnownArrayBuffer();
  t.assertEqual(0, wellKnown.byteLength);
});

test("array buffer equals", (t) => {
  t.assertEqual(makeBytes(16).byteLength, 16);
  t.assertEqual(makeBytes(16), makeBytes(16), undefined, u8Equals);

  const mutated = new Uint8Array(
    new Uint32Array(makeBytes(32).buffer, 0).reverse().buffer,
  );
  t.assertNotEqual(mutated, makeBytes(32), undefined, u8Equals);
});

test("array buffer roundtrip using lift/lower", (t) => {
  function rt(u8: Uint8Array) {
    t.assertEqual(u8, identityArrayBuffer(u8), undefined, u8Equals);
  }
  for (let i = 0; i < 64; i++) {
    rt(makeBytes(i));
  }
});

test("array buffer roundtrip using read/write", (t) => {
  function rt(u8: Uint8Array) {
    t.assertEqual(u8, identityArrayBufferForcedRead(u8)!, undefined, u8Equals);
  }
  for (let i = 0; i < 64; i++) {
    rt(makeBytes(i));
  }
});

test("ArrayBuffer roundtrip of different sizes", (t) => {
  function rt(u8: Uint8Array) {
    t.assertNotNull(identityArrayBuffer(u8)!);
  }
  // 1 kB = 1<<10
  // 1 MB = 1<<20
  // 16 MB = 1<<24
  for (let i = 0; i < 26; i++) {
    const byteLength = 1 << i;
    const buffer = new Uint8Array(byteLength);
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
  function rt(u8: Uint8Array) {
    t.assertEqual(
      u8,
      identityArrayBuffer(new Uint8Array(new Uint32Array(u8.buffer).buffer)),
      undefined,
      u8Equals,
    );
  }
  for (let i = 0; i < 64; i += 4) {
    rt(makeBytes(i));
  }
});

xtest("array buffer roundtrip with ArrayBufferView of different sizes", (t) => {
  // Typescript before 5.7, accepted typed arrays as ArrayBuffer.
  // This is no longer the case.
  // Now: ArrayBufferView is a distinct union type.
  function rt(viewName: string, ta: ArrayBufferView, slice: Uint8Array) {
    t.assertEqual(
      slice,
      identityArrayBuffer(slice),
      `${viewName} didn't match`,
      u8Equals,
    );
  }
  const base = makeBytes(64);
  const baseBuffer = base.buffer as ArrayBuffer;
  const arrayTypes = [
    {
      name: "Uint8Array",
      bytesPerElement: Uint8Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Uint8Array(ab, byteOffset, length),
    },
    {
      name: "Uint16Array",
      bytesPerElement: Uint16Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Uint16Array(ab, byteOffset, length),
    },
    {
      name: "Uint32Array",
      bytesPerElement: Uint32Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Uint32Array(ab, byteOffset, length),
    },
    {
      name: "BigUint64Array",
      bytesPerElement: BigUint64Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new BigUint64Array(ab, byteOffset, length),
    },
    {
      name: "Int8Array",
      bytesPerElement: Int8Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Int8Array(ab, byteOffset, length),
    },
    {
      name: "Int16Array",
      bytesPerElement: Int16Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Int16Array(ab, byteOffset, length),
    },
    {
      name: "Int32Array",
      bytesPerElement: Int32Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Int32Array(ab, byteOffset, length),
    },
    {
      name: "BigInt64Array",
      bytesPerElement: BigInt64Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new BigInt64Array(ab, byteOffset, length),
    },
    {
      name: "Float32Array",
      bytesPerElement: Float32Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Float32Array(ab, byteOffset, length),
    },
    {
      name: "Float64Array",
      bytesPerElement: Float64Array.BYTES_PER_ELEMENT,
      new: (ab: ArrayBuffer, byteOffset: number, length: number) =>
        new Float64Array(ab, byteOffset, length),
    },
  ];
  for (const arrayType of arrayTypes) {
    const bytesPerElement = arrayType.bytesPerElement;
    const viewName = arrayType.name;
    for (
      let byteLength = bytesPerElement;
      byteLength < base.byteLength;
      byteLength += bytesPerElement
    ) {
      for (
        let byteOffset = 0;
        byteOffset + byteLength < base.byteLength;
        byteOffset += byteLength
      ) {
        const typedArray = arrayType.new(
          baseBuffer,
          byteOffset,
          byteLength / bytesPerElement,
        );
        const slice = new Uint8Array(base.buffer, byteOffset, byteLength);
        rt(viewName, typedArray, slice);
      }
    }
  }
});

function u8Equals(a: Uint8Array, b: Uint8Array): boolean {
  if (a.byteLength !== b.byteLength) {
    return false;
  }

  const len = a.byteLength;

  for (let i = 0; i < len; i++) {
    if (a.at(i) !== b.at(i)) {
      return false;
    }
  }

  return true;
}

function makeBytes(numBytes: number): Uint8Array {
  return Uint8Array.from({ length: numBytes }, (_v, i) => i % 255);
}

test("Test nested optionals", (t) => {
  t.assertEqual("foo", identityNestedOptional("foo"));
  t.assertEqual(undefined, identityNestedOptional(undefined));

  // We can model None…
  t.assertEqual(0, matchNestedOptional(undefined));
  // …and Some(Some(_))
  t.assertEqual(2, matchNestedOptional("foo"));
  // but not Some(None).
});
