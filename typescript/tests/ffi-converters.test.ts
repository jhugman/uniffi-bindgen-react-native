/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { RustBuffer } from "../src/ffi-types";
import {
  FfiConverter,
  FfiConverterArray,
  AbstractFfiConverterArrayBuffer,
  FfiConverterBool,
  FfiConverterInt16,
  FfiConverterInt32,
  FfiConverterInt8,
  FfiConverterOptional,
  FfiConverterPrimitive,
  FfiConverterUInt16,
  FfiConverterUInt8,
} from "../src/ffi-converters";
import { Asserts, test } from "../testing/asserts";

class TestConverter<
  R extends any,
  T,
> extends AbstractFfiConverterArrayBuffer<T> {
  constructor(public inner: FfiConverter<R, T>) {
    super();
  }
  read(from: RustBuffer): T {
    return this.inner.read(from);
  }
  write(value: T, into: RustBuffer): void {
    return this.inner.write(value, into);
  }
  allocationSize(value: T): number {
    return this.inner.allocationSize(value);
  }
}

function testConverter<T>(
  t: Asserts,
  converter: AbstractFfiConverterArrayBuffer<T>,
  input: T,
) {
  const lowered = converter.lower(input);
  t.assertEqual(
    lowered.byteLength,
    converter.allocationSize(input),
    "allocated size doesn't match",
  );
  const output = converter.lift(lowered);
  t.assertEqual(input, output, "Round trip failed");
}

test("1 byte converter", (t) => {
  const converter = new TestConverter(FfiConverterInt8);
  testConverter(t, converter, -0x7f);
  testConverter(t, converter, 0);
  testConverter(t, converter, 0x7f);
});

test("boolean converter", (t) => {
  const converter = new TestConverter(FfiConverterBool);
  testConverter(t, converter, true);
  testConverter(t, converter, false);
});

test("2 byte converter", (t) => {
  const converter = new TestConverter(FfiConverterInt16);
  testConverter(t, converter, -0x7fff);
  testConverter(t, converter, 0);
  testConverter(t, converter, 0x7fff);
});

test("4 byte converter", (t) => {
  const converter = new TestConverter(FfiConverterInt32);
  testConverter(t, converter, -0x7fffffff);
  testConverter(t, converter, 0);
  testConverter(t, converter, 0x7fffffff);
});

test("Optional 1 byte converter", (t) => {
  const converter = new FfiConverterOptional(FfiConverterInt8);
  testConverter(t, converter, -0x7f);
  testConverter(t, converter, 0);
  testConverter(t, converter, 0x7f);
  testConverter(t, converter, undefined);
});

test("Optional 2 byte converter (mixed width byte array)", (t) => {
  const converter = new FfiConverterOptional(FfiConverterInt16);
  testConverter(t, converter, -0x7fff);
  testConverter(t, converter, 0);
  testConverter(t, converter, 0x7fff);
  testConverter(t, converter, undefined);
});

test("Array of bytes", (t) => {
  const converter = new FfiConverterArray(FfiConverterUInt8);
  testConverter(t, converter, [1, 2, 3]);
  testConverter(t, converter, [0]);
  testConverter(t, converter, new Array(100).fill(128));
});

test("Array of shorts", (t) => {
  const converter = new FfiConverterArray(FfiConverterUInt16);
  testConverter(t, converter, [1, 2, 3]);
  testConverter(t, converter, [0]);
  testConverter(t, converter, new Array(100).fill(128));
});

test("Array of optional shorts", (t) => {
  const converter = new FfiConverterArray(
    new FfiConverterOptional(FfiConverterUInt16),
  );
  testConverter(t, converter, [1, 2, 3]);
  testConverter(t, converter, [0]);
  testConverter(t, converter, [0, undefined, 1]);
  testConverter(t, converter, new Array(100).fill(128));
  testConverter(t, converter, new Array(100).fill(undefined));
});
