/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  FfiConverterObjectWithCallbacks,
  type UniffiObjectFactory,
} from "../src/objects";
import { type UniffiHandle, UniffiHandleMap } from "../src/handle-map";
import { test } from "../testing/asserts";

// Rust-backed objects cross the FFI as Arc pointers, which are always even.
// Foreign (TS-implemented) objects cross as handle-map keys, which are always
// odd. `lift` must decide which it has been given by the low bit, never by
// whether the handle happens to be in the map: after a hot reload the map is
// new and empty, but Rust still holds the old runtime's odd handles.

class Native {
  constructor(readonly pointer: UniffiHandle) {}
}
class Foreign {}
type Obj = Native | Foreign;

function fakeFactory(created: UniffiHandle[]): UniffiObjectFactory<Obj> {
  return {
    bless: () => ({}) as any,
    unbless: () => {},
    create: (pointer) => {
      created.push(pointer);
      return new Native(pointer);
    },
    pointer: (obj) => (obj as Native).pointer,
    clonePointer: (obj) => (obj as Native).pointer,
    freePointer: () => {},
    isConcreteType: (obj): obj is Native => obj instanceof Native,
  };
}

// Matched by message, as handlemap.test.ts does: under Metro the bundle can hold
// two instances of errors.ts, so `instanceof` is not reliable there.
const isStale = (e: any) =>
  e instanceof Error && e.message.includes("handle map");
const noAlloc = undefined as any;

test("a foreign object round-trips through its own handle map", (t) => {
  const created: UniffiHandle[] = [];
  const converter = new FfiConverterObjectWithCallbacks<Obj>(
    fakeFactory(created),
  );
  const obj = new Foreign();
  const handle = converter.lower(obj, noAlloc);
  t.assertTrue(handle % BigInt(2) === BigInt(1), "foreign handles are odd");
  t.assertTrue(converter.lift(handle) === obj, "same object back");
  t.assertEqual(created.length, 0, "the pointer path was never taken");
});

test("an even handle is a Rust pointer and never consults the map", (t) => {
  const created: UniffiHandle[] = [];
  const converter = new FfiConverterObjectWithCallbacks<Obj>(
    fakeFactory(created),
  );
  const lifted = converter.lift(BigInt(8));
  t.assertTrue(lifted instanceof Native);
  t.assertEqual(created, [BigInt(8)]);
});

test("a stale foreign handle throws rather than being lifted as a pointer", (t) => {
  const created: UniffiHandle[] = [];
  const converter = new FfiConverterObjectWithCallbacks<Obj>(
    fakeFactory(created),
  );
  // Odd, and in no map: exactly what Rust hands back after a reload.
  t.assertThrows(isStale, () => converter.lift(BigInt(1)));
  t.assertEqual(
    created.length,
    0,
    "must not hand a foreign handle to factory.create as if it were an Arc",
  );
});

test("a handle minted by a previous runtime's map is stale in the next", (t) => {
  // The reload scenario: the object was lowered before the reload, and Rust
  // still holds the handle; the converter after the reload has a fresh map.
  const before = new FfiConverterObjectWithCallbacks<Obj>(
    fakeFactory([]),
    new UniffiHandleMap<Obj>(),
  );
  const handle = before.lower(new Foreign(), noAlloc);

  const created: UniffiHandle[] = [];
  const after = new FfiConverterObjectWithCallbacks<Obj>(
    fakeFactory(created),
    new UniffiHandleMap<Obj>(),
  );
  t.assertThrows(isStale, () => after.lift(handle));
  t.assertEqual(created.length, 0);
});
