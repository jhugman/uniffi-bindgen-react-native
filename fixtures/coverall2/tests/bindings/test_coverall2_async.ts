/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-coverall2 -- async_wasm

import {
  identityArrayBuffer,
  identityArrayBufferForcedRead,
  identityNestedOptional,
  matchNestedOptional,
  wellKnownArrayBuffer,
} from "@/generated/uniffi_coverall2";
import { asyncTest, xasyncTest } from "@/asserts";
import "@/polyfills";

(async () => {
  await asyncTest("well known array buffer returned", async (t) => {
    const wellKnown = await wellKnownArrayBuffer();
    t.assertEqual(0, wellKnown.byteLength);
    t.end();
  });

  await asyncTest("array buffer equals", async (t) => {
    t.assertEqual(arrayBuffer(16).byteLength, 16);
    t.assertEqual(arrayBuffer(16), arrayBuffer(16), undefined, abEquals);

    const mutated = new Uint32Array(arrayBuffer(32), 0).reverse().buffer;
    t.assertNotEqual(mutated, arrayBuffer(32), undefined, abEquals);
    t.end();
  });

  await asyncTest("array buffer roundtrip using lift/lower", async (t) => {
    async function rt(ab: ArrayBuffer) {
      t.assertEqual(ab, await identityArrayBuffer(ab), undefined, abEquals);
    }
    for (let i = 0; i < 64; i++) {
      await rt(arrayBuffer(i));
    }
    t.end();
  });

  await asyncTest("array buffer roundtrip using read/write", async (t) => {
    async function rt(ab: ArrayBuffer) {
      t.assertEqual(
        ab,
        (await identityArrayBufferForcedRead(ab))!,
        undefined,
        abEquals,
      );
    }
    for (let i = 0; i < 64; i++) {
      await rt(arrayBuffer(i));
    }
    t.end();
  });

  await asyncTest(
    "ArrayBuffer roundtrip of different sizes",
    async (t) => {
      async function rt(ab: ArrayBuffer) {
        t.assertNotNull(await identityArrayBuffer(ab)!);
      }
      // 1 kB = 1<<10
      // 1 MB = 1<<20
      // 16 MB = 1<<24
      for (let i = 0; i < 26; i++) {
        const byteLength = 1 << i;
        const buffer = new ArrayBuffer(byteLength);
        const start = Date.now();
        await rt(buffer);
        const end = Date.now();
        console.log(
          `ArrayBuffer roundtrip: ${bytes(byteLength)} in ${end - start} ms`,
        );
      }
      t.end();
    },
    120000,
  );
  await asyncTest("array buffer roundtrip with ArrayBufferView", async (t) => {
    async function rt(ab: ArrayBuffer) {
      t.assertEqual(
        ab,
        await identityArrayBuffer(new Uint32Array(ab).buffer),
        undefined,
        abEquals,
      );
    }
    for (let i = 0; i < 64; i += 4) {
      await rt(arrayBuffer(i));
    }
    t.end();
  });

  await xasyncTest(
    "array buffer roundtrip with ArrayBufferView of different sizes",
    async (t) => {
      // Typescript before 5.7, accepted typed arrays as ArrayBuffer.
      // This is no longer the case.
      // Now: ArrayBufferView is a distinct union type.
      async function rt(
        viewName: string,
        ta: ArrayBufferView,
        slice: ArrayBuffer,
      ) {
        t.assertEqual(
          slice,
          await identityArrayBuffer(slice),
          `${viewName} didn't match`,
          abEquals,
        );
      }
      const base = arrayBuffer(64);
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
              base,
              byteOffset,
              byteLength / bytesPerElement,
            );
            const slice = base.slice(byteOffset, byteOffset + byteLength);
            await rt(viewName, typedArray, slice);
          }
        }
      }
      t.end();
    },
  );

  await asyncTest("Test nested optionals", async (t) => {
    t.assertEqual("foo", await identityNestedOptional("foo"));
    t.assertEqual(undefined, await identityNestedOptional(undefined));

    // We can model None…
    t.assertEqual(0, await matchNestedOptional(undefined));
    // …and Some(Some(_))
    t.assertEqual(2, await matchNestedOptional("foo"));
    // but not Some(None).
    t.end();
  });
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
