/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-array-buffers -- jsi
//   cargo test -p uniffi-fixture-array-buffers -- wasm
//
// Sibling to `strict-byte-arrays`'s test suite: that fixture forces `Vec<u8>`
// to be emitted as `Uint8Array`; this fixture leaves the default config in
// place, so these same roundtrips exercise the plain `ArrayBuffer` /
// `FfiConverterArrayBuffer` cursor path instead.

import {
  identityBytes,
  identityBytesForcedRead,
  wellKnownBytes,
} from "@/generated/uniffi_array_buffers";
import { test } from "@/asserts";
import "@/polyfills";

test("well known array returned", (t) => {
  const wellKnown = wellKnownBytes();
  t.assertTrue(wellKnown instanceof ArrayBuffer, "result is an ArrayBuffer");
  t.assertEqual(4, wellKnown.byteLength);
  t.assertEqual(
    new Uint8Array([1, 2, 3, 255]),
    new Uint8Array(wellKnown),
    undefined,
    byteArrayEquals,
  );
});

test("array roundtrip using lift/lower", (t) => {
  function rt(ab: ArrayBuffer) {
    const result = identityBytes(ab);
    t.assertTrue(result instanceof ArrayBuffer, "result is an ArrayBuffer");
    t.assertEqual(
      new Uint8Array(ab),
      new Uint8Array(result),
      undefined,
      byteArrayEquals,
    );
  }
  for (let i = 0; i < 64; i++) {
    rt(arrayBuffer(i));
  }
});

test("array roundtrip using read/write", (t) => {
  function rt(ab: ArrayBuffer) {
    const result = identityBytesForcedRead(ab)!;
    t.assertTrue(result instanceof ArrayBuffer, "result is an ArrayBuffer");
    t.assertEqual(
      new Uint8Array(ab),
      new Uint8Array(result),
      undefined,
      byteArrayEquals,
    );
  }
  for (let i = 0; i < 64; i++) {
    rt(arrayBuffer(i));
  }
});

test("ArrayBuffer roundtrip of different sizes", (t) => {
  function rt(ab: ArrayBuffer) {
    const result = identityBytes(ab);
    // Avoid assertions that would stringify the buffer (e.g. via a template
    // literal in the failure message) — that's O(N) per call and thrashes
    // the Hermes GC at MB scale.
    t.assertTrue(result !== null && result !== undefined);
    t.assertEqual(result!.byteLength, ab.byteLength);
  }
  // 1 kB = 1<<10
  // 1 MB = 1<<20
  // 16 MB = 1<<24
  for (let i = 0; i < 26; i++) {
    const byteLength = 1 << i;
    const buffer = arrayBuffer(byteLength);
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

function arrayBuffer(numBytes: number): ArrayBuffer {
  return Uint8Array.from({ length: numBytes }, (_v, i) => i % 255).buffer;
}
