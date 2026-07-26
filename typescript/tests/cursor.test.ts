/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { Cursor } from "../src/cursor";
import { test } from "../testing/asserts";

test("Cursor: u8/i8 round-trip at boundaries", (t) => {
  const buf = new ArrayBuffer(8);
  const w = new Cursor(buf, 0, 8);
  w.writeU8(0);
  w.writeU8(255);
  w.writeI8(-128);
  w.writeI8(127);
  const r = new Cursor(buf, 0, 8);
  t.assertEqual(r.readU8(), 0);
  t.assertEqual(r.readU8(), 255);
  t.assertEqual(r.readI8(), -128);
  t.assertEqual(r.readI8(), 127);
});

test("Cursor: u32/i32 big-endian round-trip", (t) => {
  const buf = new ArrayBuffer(16);
  const w = new Cursor(buf, 0, 16);
  w.writeU32(0xdeadbeef);
  w.writeI32(-2_147_483_648);
  w.writeI32(2_147_483_647);
  w.writeU32(0);
  const r = new Cursor(buf, 0, 16);
  t.assertEqual(r.readU32(), 0xdeadbeef);
  t.assertEqual(r.readI32(), -2_147_483_648);
  t.assertEqual(r.readI32(), 2_147_483_647);
  t.assertEqual(r.readU32(), 0);
});

test("Cursor: u64/i64 bigint round-trip", (t) => {
  // ts target is es5 in the test harness, so use BigInt() instead of `n` literals.
  const U64_MAX = BigInt("0xffffffffffffffff");
  const I64_MIN = BigInt("-9223372036854775808");
  const I64_MAX = BigInt("9223372036854775807");
  const ZERO = BigInt(0);
  const buf = new ArrayBuffer(32);
  const w = new Cursor(buf, 0, 32);
  w.writeU64(U64_MAX);
  w.writeI64(I64_MIN);
  w.writeI64(I64_MAX);
  w.writeU64(ZERO);
  const r = new Cursor(buf, 0, 32);
  t.assertEqual(r.readU64(), U64_MAX);
  t.assertEqual(r.readI64(), I64_MIN);
  t.assertEqual(r.readI64(), I64_MAX);
  t.assertEqual(r.readU64(), ZERO);
});

test("Cursor: f32/f64 round-trip", (t) => {
  const buf = new ArrayBuffer(12);
  const w = new Cursor(buf, 0, 12);
  w.writeF32(1.5);
  w.writeF64(Math.PI);
  const r = new Cursor(buf, 0, 12);
  t.assertEqual(r.readF32(), 1.5);
  t.assertEqual(r.readF64(), Math.PI);
});

test("Cursor: bool maps to 0/1 bytes", (t) => {
  const buf = new ArrayBuffer(2);
  const w = new Cursor(buf, 0, 2);
  w.writeBool(true);
  w.writeBool(false);
  const r = new Cursor(buf, 0, 2);
  t.assertEqual(r.readBool(), true);
  t.assertEqual(r.readBool(), false);
});

test("Cursor: readBytes returns a fresh copy, not an alias", (t) => {
  const buf = new ArrayBuffer(8);
  new Uint8Array(buf).set([1, 2, 3, 4, 5, 6, 7, 8]);
  const r = new Cursor(buf, 2, 4); // window from offset 2, 4 bytes
  const bytes = r.readBytes(4);
  t.assertEqual(Array.from(bytes), [3, 4, 5, 6]);
  // mutating the original buffer doesn't affect the returned bytes
  new Uint8Array(buf).fill(0);
  t.assertEqual(Array.from(bytes), [3, 4, 5, 6]);
});

test("Cursor: writeBytes copies src into the cursor's region", (t) => {
  const buf = new ArrayBuffer(8);
  const w = new Cursor(buf, 1, 4);
  w.writeBytes(new Uint8Array([0xaa, 0xbb, 0xcc, 0xdd]));
  t.assertEqual(
    Array.from(new Uint8Array(buf)),
    [0, 0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0],
  );
});

test("Cursor: skip advances pos without reading", (t) => {
  const buf = new ArrayBuffer(8);
  new Uint8Array(buf).set([0, 0, 0, 0, 0x42, 0, 0, 0]);
  const r = new Cursor(buf, 0, 8);
  r.skip(4);
  t.assertEqual(r.readU8(), 0x42);
});

test("Cursor: remaining() reports unread bytes", (t) => {
  const buf = new ArrayBuffer(8);
  const r = new Cursor(buf, 0, 8);
  t.assertEqual(r.remaining(), 8);
  r.readU32();
  t.assertEqual(r.remaining(), 4);
});

test("Cursor: throws on read past end", (t) => {
  const buf = new ArrayBuffer(4);
  const r = new Cursor(buf, 0, 4);
  r.readU32();
  t.assertThrows(
    (e) => e instanceof Error,
    () => r.readU8(),
  );
});
