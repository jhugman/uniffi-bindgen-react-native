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
  const array = Uint8Array.from({ length: numBytes }, (_v, i) => i);
  return array.buffer;
}
