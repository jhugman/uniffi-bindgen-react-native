/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- async_wasm::test_strict_byte_arrays

import {
  identityBytes,
  identityBytesForcedRead,
  wellKnownBytes,
} from "@/generated/uniffi_strict_byte_arrays";
import { asyncTest } from "@/asserts";
import "@/polyfills";

(async () => {
  await asyncTest("well known array returned", async (t) => {
    const wellKnown = await wellKnownBytes();
    t.assertEqual(4, wellKnown.byteLength);
    t.assertEqual(new Uint8Array([1, 2, 3, 255]), wellKnown);
    t.end();
  });

  await asyncTest("array equals", async (t) => {
    t.assertEqual(uint8Array(16).byteLength, 16);
    t.assertEqual(uint8Array(16), uint8Array(16), undefined, byteArrayEquals);

    const mutated = new Uint8Array(uint8Array(32).buffer, 0).reverse();
    t.assertNotEqual(mutated, uint8Array(32), undefined, byteArrayEquals);
    t.end();
  });

  await asyncTest("array roundtrip using lift/lower", async (t) => {
    async function rt(ab: Uint8Array) {
      t.assertEqual(ab, await identityBytes(ab), undefined, byteArrayEquals);
    }
    for (let i = 0; i < 64; i++) {
      await rt(uint8Array(i));
    }
    t.end();
  });

  await asyncTest("array roundtrip using read/write", async (t) => {
    async function rt(ab: Uint8Array) {
      t.assertEqual(
        ab,
        (await identityBytesForcedRead(ab))!,
        undefined,
        byteArrayEquals,
      );
    }
    for (let i = 0; i < 64; i++) {
      await rt(uint8Array(i));
    }
    t.end();
  });

  await asyncTest(
    "Uint8Array roundtrip of different sizes",
    async (t) => {
      async function rt(ab: Uint8Array) {
        const result = await identityBytes(ab);
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
        await rt(buffer);
        const end = Date.now();
        console.log(
          `Uint8Array roundtrip: ${bytes(byteLength)} in ${end - start} ms`,
        );
      }
      t.end();
    },
    // Headroom for 32 MB copies through the Worker on slow CI.
    120000,
  );
})();

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
