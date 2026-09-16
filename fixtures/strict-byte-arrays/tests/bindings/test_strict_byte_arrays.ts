/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- jsi
//   cargo test -p uniffi-fixture-strict-byte-arrays -- wasm

import {
  BorrowedBytes,
  borrowedBytesChecksum,
  concatBorrowedBytes,
  copyBorrowedBytes,
  mixOwnedAndBorrowedBytes,
  identityBytes,
  identityBytesForcedRead,
  wellKnownBytes,
} from "@/generated/uniffi_strict_byte_arrays";
import { test } from "@/asserts";
import "@/polyfills";

test("well known array returned", (t) => {
  const wellKnown = wellKnownBytes();
  t.assertEqual(4, wellKnown.byteLength);
  t.assertEqual(new Uint8Array([1, 2, 3, 255]), wellKnown);
});

test("array equals", (t) => {
  t.assertEqual(uint8Array(16).byteLength, 16);
  t.assertEqual(uint8Array(16), uint8Array(16), undefined, byteArrayEquals);

  const mutated = new Uint8Array(uint8Array(32).buffer, 0).reverse();
  t.assertNotEqual(mutated, uint8Array(32), undefined, byteArrayEquals);
});

test("array roundtrip using lift/lower", (t) => {
  function rt(ab: Uint8Array) {
    t.assertEqual(ab, identityBytes(ab), undefined, byteArrayEquals);
  }
  for (let i = 0; i < 64; i++) {
    rt(uint8Array(i));
  }
});

test("array roundtrip using read/write", (t) => {
  function rt(ab: Uint8Array) {
    t.assertEqual(ab, identityBytesForcedRead(ab)!, undefined, byteArrayEquals);
  }
  for (let i = 0; i < 64; i++) {
    rt(uint8Array(i));
  }
});

test("Uint8Array roundtrip of different sizes", (t) => {
  function rt(ab: Uint8Array) {
    const result = identityBytes(ab);
    // Avoid assertions that would stringify the Uint8Array (e.g. via a
    // template literal in the failure message) — that's O(N) per call and
    // thrashes the Hermes GC at MB scale.
    t.assertTrue(result !== null && result !== undefined);
    t.assertEqual(result!.byteLength, ab.byteLength);
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
      `Uint8Array roundtrip: ${bytes(byteLength)} in ${end - start} ms`,
    );
  }
});

test("borrowed bytes consume empty and content inputs", (t) => {
  for (const value of [new Uint8Array(), new Uint8Array([0, 1, 128, 255])]) {
    t.assertEqual(value.length === 0 ? 0 : 384, borrowedBytesChecksum(value));
    t.assertEqual(
      value,
      new Uint8Array(copyBorrowedBytes(value)),
      undefined,
      byteArrayEquals,
    );
  }
});

test("multiple borrowed and mixed owned byte arguments retain order", (t) => {
  const first = new Uint8Array([1, 2]);
  const middle = new Uint8Array([128]);
  const last = new Uint8Array([3, 255]);
  const empty = new Uint8Array();
  t.assertEqual(
    new Uint8Array([1, 2, 3, 255]),
    new Uint8Array(concatBorrowedBytes(first, last)),
    undefined,
    byteArrayEquals,
  );
  t.assertEqual(
    new Uint8Array([1, 2, 128, 3, 255]),
    new Uint8Array(mixOwnedAndBorrowedBytes(first, middle, last)),
    undefined,
    byteArrayEquals,
  );
  t.assertEqual(0, concatBorrowedBytes(empty, empty).byteLength);
  t.assertEqual(
    last,
    new Uint8Array(mixOwnedAndBorrowedBytes(empty, empty, last)),
    undefined,
    byteArrayEquals,
  );
});

test("borrowed constructor copies input and method consumes bytes", (t) => {
  const prefix = new Uint8Array([1, 2]);
  const suffix = new Uint8Array([3, 255]);
  const empty = new Uint8Array();
  const consumer = new BorrowedBytes(prefix);
  try {
    prefix.fill(99);
    t.assertEqual(
      new Uint8Array([1, 2, 3, 255]),
      new Uint8Array(consumer.append(suffix)),
      undefined,
      byteArrayEquals,
    );
    t.assertEqual(
      new Uint8Array([1, 2]),
      new Uint8Array(consumer.append(empty)),
      undefined,
      byteArrayEquals,
    );
  } finally {
    consumer.uniffiDestroy();
  }
  const emptyConsumer = new BorrowedBytes(empty);
  try {
    t.assertEqual(0, emptyConsumer.append(empty).byteLength);
  } finally {
    emptyConsumer.uniffiDestroy();
  }
});

test("borrowed Uint8Array subarrays exclude offset and trailing sentinels", (t) => {
  const backing = new Uint8Array([201, 202, 1, 2, 203, 3, 255, 204]);
  const first = backing.subarray(2, 4);
  const last = backing.subarray(5, 7);
  const empty = backing.subarray(3, 3);
  t.assertEqual(3, borrowedBytesChecksum(first));
  t.assertEqual(258, borrowedBytesChecksum(last));
  t.assertEqual(0, borrowedBytesChecksum(empty));
  t.assertEqual(
    new Uint8Array([1, 2]),
    copyBorrowedBytes(first),
    undefined,
    byteArrayEquals,
  );
  t.assertEqual(
    new Uint8Array([1, 2, 3, 255]),
    concatBorrowedBytes(first, last),
    undefined,
    byteArrayEquals,
  );
  t.assertEqual(
    new Uint8Array([1, 2, 3, 255, 1, 2]),
    mixOwnedAndBorrowedBytes(first, last, first),
    undefined,
    byteArrayEquals,
  );
  t.assertEqual(0, copyBorrowedBytes(empty).byteLength);
  const consumer = new BorrowedBytes(first);
  try {
    t.assertEqual(
      new Uint8Array([1, 2, 3, 255]),
      consumer.append(last),
      undefined,
      byteArrayEquals,
    );
  } finally {
    consumer.uniffiDestroy();
  }
  t.assertEqual(
    new Uint8Array([201, 202, 1, 2, 203, 3, 255, 204]),
    backing,
    undefined,
    byteArrayEquals,
  );
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

function byteArrayEquals(a: Uint8Array, b: Uint8Array): boolean {
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

function uint8Array(numBytes: number): Uint8Array {
  return Uint8Array.from({ length: numBytes }, (_v, i) => i % 255);
}
