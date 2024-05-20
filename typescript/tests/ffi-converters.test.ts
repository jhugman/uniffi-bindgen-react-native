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
import { assertEqual, test } from "../testing/asserts";

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
  converter: AbstractFfiConverterArrayBuffer<T>,
  input: T,
) {
  const lowered = converter.lower(input);
  assertEqual(
    lowered.byteLength,
    converter.allocationSize(input),
    "allocated size doesn't match",
  );
  const output = converter.lift(lowered);
  assertEqual(input, output, "Round trip failed");
}

test("1 byte converter", () => {
  const converter = new TestConverter(FfiConverterInt8);
  testConverter(converter, -(1 << 7) + 1);
  testConverter(converter, 0);
  testConverter(converter, (1 << 7) - 1);
});

test("boolean converter", () => {
  const converter = new TestConverter(FfiConverterBool);
  testConverter(converter, true);
  testConverter(converter, false);
});

test("2 byte converter", () => {
  const converter = new TestConverter(FfiConverterInt16);
  testConverter(converter, -(1 << 15) + 1);
  testConverter(converter, 0);
  testConverter(converter, (1 << 15) - 1);
});

test("4 byte converter", () => {
  const converter = new TestConverter(FfiConverterInt32);
  testConverter(converter, -(1 << 30));
  testConverter(converter, 0);
  testConverter(converter, 1 << 30);
});

test("Optional 1 byte converter", () => {
  const converter = new FfiConverterOptional(FfiConverterInt8);
  testConverter(converter, -(1 << 7) + 1);
  testConverter(converter, 0);
  testConverter(converter, (1 << 7) - 1);
  testConverter(converter, undefined);
});

test("Optional 2 byte converter (mixed width byte array)", () => {
  const converter = new FfiConverterOptional(FfiConverterInt16);
  testConverter(converter, -(1 << 15) + 1);
  testConverter(converter, 0);
  testConverter(converter, (1 << 15) - 1);
  testConverter(converter, undefined);
});

test("Array of bytes", () => {
  const converter = new FfiConverterArray(FfiConverterUInt8);
  testConverter(converter, [1, 2, 3]);
  testConverter(converter, [0]);
  testConverter(converter, new Array(100).fill(128));
});

test("Array of shorts", () => {
  const converter = new FfiConverterArray(FfiConverterUInt16);
  testConverter(converter, [1, 2, 3]);
  testConverter(converter, [0]);
  testConverter(converter, new Array(100).fill(128));
});

test("Array of optional shorts", () => {
  const converter = new FfiConverterArray(
    new FfiConverterOptional(FfiConverterUInt16),
  );
  testConverter(converter, [1, 2, 3]);
  testConverter(converter, [0]);
  testConverter(converter, [0, undefined, 1]);
  testConverter(converter, new Array(100).fill(128));
  testConverter(converter, new Array(100).fill(undefined));
});
