/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { Memory } from "../src/memory.js";

function makeMem(initialPages = 1) {
  return new WebAssembly.Memory({ initial: initialPages });
}

test("readU32 / writeU32 round-trip", () => {
  const m = new Memory(makeMem());
  m.writeU32(0x100, 0xdeadbeef);
  assert.strictEqual(m.readU32(0x100), 0xdeadbeef);
});

test("readU64 / writeU64 round-trip via bigint", () => {
  const m = new Memory(makeMem());
  const v = 0x0102030405060708n;
  m.writeU64(0x200, v);
  assert.strictEqual(m.readU64(0x200), v);
});

test("readBytes returns a copy, not a live view", () => {
  const m = new Memory(makeMem());
  m.writeU32(0x300, 0xaabbccdd);
  const copy = m.readBytes(0x300, 4);
  m.writeU32(0x300, 0);
  assert.deepStrictEqual(Array.from(copy), [0xdd, 0xcc, 0xbb, 0xaa]);
});

test("views are re-acquired after memory.grow()", () => {
  const wm = makeMem(1);
  const m = new Memory(wm);
  // Touch the view first so it caches.
  m.writeU32(0x100, 1);
  wm.grow(1);
  // After grow, old buffer is detached; reading must NOT throw and must
  // read the freshly written value.
  m.writeU32(0x100, 0xfeedface);
  assert.strictEqual(m.readU32(0x100), 0xfeedface);
});
