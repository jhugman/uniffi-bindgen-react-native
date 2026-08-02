/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { Scratch } from "../src/scratch.js";

function makeFakeAllocator(arenaPtr: number, arenaSize: number) {
  const allocs = new Map<number, number>(); // ptr -> size (for overflow)
  let nextPtr = 0x10000;
  return {
    arenaPtr,
    arenaSize,
    alloc(size: number) {
      const p = nextPtr;
      nextPtr += size;
      allocs.set(p, size);
      return p;
    },
    free(ptr: number, size: number) {
      assert.strictEqual(allocs.get(ptr), size, "free with wrong size");
      allocs.delete(ptr);
    },
    pendingAllocs: () => allocs.size,
  };
}

test("push/pop returns pointers within the arena", () => {
  const a = makeFakeAllocator(0x1000, 1024);
  const s = new Scratch(a.arenaPtr, a.arenaSize, a.alloc, a.free);
  const p = s.push(64);
  assert.ok(p >= 0x1000 && p < 0x1000 + 1024);
  s.pop(p);
});

test("nested push/pop preserves stack discipline", () => {
  const a = makeFakeAllocator(0x1000, 1024);
  const s = new Scratch(a.arenaPtr, a.arenaSize, a.alloc, a.free);
  const p1 = s.push(64);
  const p2 = s.push(128);
  assert.ok(p2 >= p1 + 64);
  s.pop(p2);
  s.pop(p1);
});

test("overflow falls back to allocator and free returns it", () => {
  const a = makeFakeAllocator(0x1000, 64);
  const s = new Scratch(a.arenaPtr, a.arenaSize, a.alloc, a.free);
  const p = s.push(128); // larger than arena
  assert.strictEqual(a.pendingAllocs(), 1);
  s.pop(p);
  assert.strictEqual(a.pendingAllocs(), 0);
});

test("Scratch.reserve carves a permanent region", () => {
  const memory: number[] = [];
  const alloc = (n: number) => {
    memory.push(n);
    return 0x1000 + memory.length * 1000;
  };
  const free = (_p: number, _n: number) => {};
  const arena = new Scratch(0x1000, 256, alloc, free);

  const slot1 = arena.reserve(24);
  const slot2 = arena.reserve(32);

  // Reserved regions sit at the front of the arena.
  assert.strictEqual(slot1, 0x1000);
  assert.strictEqual(slot2, 0x1000 + 24);

  // push() picks up after the reserved region.
  const tmp = arena.push(16);
  assert.strictEqual(tmp, 0x1000 + 24 + 32);

  // pop() must not invalidate reserved regions.
  arena.pop(tmp);
  const tmp2 = arena.push(8);
  assert.strictEqual(tmp2, 0x1000 + 24 + 32);
});

test("Scratch.reserve refuses growth after first push", () => {
  const arena = new Scratch(
    0x1000,
    256,
    () => 0,
    () => {},
  );
  arena.push(8);
  assert.throws(() => arena.reserve(16));
});

test("Scratch caps push to maxArenaSize, overflowing to alloc", () => {
  let overflowCount = 0;
  const alloc = (n: number) => {
    overflowCount++;
    return 0x9000;
  };
  const free = (_p: number, _n: number) => {};
  const arena = new Scratch(0x1000, 256, alloc, free, /*maxArenaSize*/ 128);

  arena.push(64); // ok, fits under cap
  arena.push(64); // ok, at the cap exactly
  const p = arena.push(8); // would exceed cap → overflow to alloc
  assert.strictEqual(p, 0x9000);
  assert.strictEqual(overflowCount, 1);
});
