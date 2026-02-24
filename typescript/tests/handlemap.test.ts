/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiHandleMap } from "../src/handle-map";
import { Asserts, test } from "../testing/asserts";

function isStaleHandleError(e: any): boolean {
  return e instanceof Error && e.message.includes("handle map");
}

// uniffi 0.30 requires foreign handles to be odd (non-zero) values so that
// they can be distinguished from Rust Arc pointers, which are always even.

test("first handle is odd and non-zero", (t) => {
  const map = new UniffiHandleMap<string>();
  const h = map.insert("a");
  t.assertTrue(h !== BigInt(0), "handle must be non-zero");
  t.assertTrue(h % BigInt(2) === BigInt(1), `handle must be odd, got ${h}`);
});

test("all handles are odd", (t) => {
  const map = new UniffiHandleMap<number>();
  for (let i = 0; i < 10; i++) {
    const h = map.insert(i);
    t.assertTrue(
      h % BigInt(2) === BigInt(1),
      `handle ${h} for insert #${i} must be odd`,
    );
  }
});

test("handles are unique", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("a");
  const h2 = map.insert("b");
  const h3 = map.insert("c");
  t.assertNotEqual(h1, h2);
  t.assertNotEqual(h2, h3);
  t.assertNotEqual(h1, h3);
});

test("handles increment by 2", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("a");
  const h2 = map.insert("b");
  t.assertEqual(h2 - h1, BigInt(2), "consecutive handles must differ by 2");
});

test("get returns inserted value", (t) => {
  const map = new UniffiHandleMap<string>();
  const h = map.insert("hello");
  t.assertEqual(map.get(h), "hello");
});

test("get with unknown handle throws stale handle error", (t) => {
  const map = new UniffiHandleMap<string>();
  t.assertThrows(isStaleHandleError, () => map.get(BigInt(99)));
});

test("get after remove throws stale handle error", (t) => {
  const map = new UniffiHandleMap<string>();
  const h = map.insert("gone");
  map.remove(h);
  t.assertThrows(isStaleHandleError, () => map.get(h));
});

test("remove returns the value", (t) => {
  const map = new UniffiHandleMap<string>();
  const h = map.insert("value");
  t.assertEqual(map.remove(h), "value");
});

test("remove of already-removed handle returns undefined", (t) => {
  const map = new UniffiHandleMap<string>();
  const h = map.insert("value");
  map.remove(h);
  t.assertNull(map.remove(h));
});

test("has returns true for live handle, false after remove", (t) => {
  const map = new UniffiHandleMap<number>();
  const h = map.insert(42);
  t.assertTrue(map.has(h), "has should return true for live handle");
  map.remove(h);
  t.assertFalse(map.has(h), "has should return false after remove");
});

test("size tracks insertions and removals", (t) => {
  const map = new UniffiHandleMap<string>();
  t.assertEqual(map.size, 0);
  const h1 = map.insert("a");
  t.assertEqual(map.size, 1);
  const h2 = map.insert("b");
  t.assertEqual(map.size, 2);
  map.remove(h1);
  t.assertEqual(map.size, 1);
  map.remove(h2);
  t.assertEqual(map.size, 0);
});

// Clone semantics: uniffi 0.30 CallbackInterfaceClone creates a second handle
// pointing to the same object, so Rust can hold two independent references to
// the same JS callback object.

test("clone returns a new, distinct handle", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("shared");
  const h2 = map.clone(h1);
  t.assertNotEqual(h1, h2, "clone must return a different handle");
});

test("cloned handle is also odd", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("x");
  const h2 = map.clone(h1);
  t.assertTrue(h2 % BigInt(2) === BigInt(1), `cloned handle ${h2} must be odd`);
});

test("both original and cloned handle resolve to same object", (t) => {
  const obj = { value: 42 };
  const map = new UniffiHandleMap<typeof obj>();
  const h1 = map.insert(obj);
  const h2 = map.clone(h1);
  t.assertTrue(
    map.get(h1) === map.get(h2),
    "both handles must be same object reference",
  );
});

test("removing original handle does not invalidate the clone", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("shared");
  const h2 = map.clone(h1);
  map.remove(h1);
  t.assertEqual(
    map.get(h2),
    "shared",
    "clone must survive removal of original",
  );
});

test("removing clone does not invalidate the original", (t) => {
  const map = new UniffiHandleMap<string>();
  const h1 = map.insert("shared");
  const h2 = map.clone(h1);
  map.remove(h2);
  t.assertEqual(
    map.get(h1),
    "shared",
    "original must survive removal of clone",
  );
});

test("clone of unknown handle throws stale handle error", (t) => {
  const map = new UniffiHandleMap<string>();
  t.assertThrows(isStaleHandleError, () => map.clone(BigInt(99)));
});

test("clone increments size by one", (t) => {
  const map = new UniffiHandleMap<string>();
  map.insert("a");
  t.assertEqual(map.size, 1);
  const h = map.insert("b");
  t.assertEqual(map.size, 2);
  map.clone(h);
  t.assertEqual(map.size, 3);
});
