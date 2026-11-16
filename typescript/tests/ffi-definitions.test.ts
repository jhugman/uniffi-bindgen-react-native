/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  FfiType,
  uniffiMergeDefinitions,
  type ModuleDefinitions,
} from "../src/ffi-definitions";
import { test } from "../testing/asserts";

// A namespace's table: its own function, plus the shapes uniffi generates in every crate.
function table(ns: string): ModuleDefinitions {
  return {
    symbols: {
      rustbuffer_alloc: `ffi_${ns}_rustbuffer_alloc`,
      rustbuffer_free: `ffi_${ns}_rustbuffer_free`,
      rustbuffer_from_bytes: `ffi_${ns}_rustbuffer_from_bytes`,
    },
    functions: {
      [`uniffi_${ns}_fn_func_hello`]: {
        args: [FfiType.RustBuffer],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
    },
    callbacks: {
      ForeignFutureDroppedCallback: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    structs: {
      ForeignFutureDroppedCallbackStruct: [
        { name: "handle", type: FfiType.UInt64 },
        {
          name: "free",
          type: FfiType.Callback("ForeignFutureDroppedCallback"),
        },
      ],
    },
  };
}

const isMergeError = (name: string) => (e: any) =>
  e instanceof Error && e.message.includes(name);

test("every namespace's functions are in the merged table", (t) => {
  const merged = uniffiMergeDefinitions([table("a"), table("b"), table("c")]);
  t.assertEqual(Object.keys(merged.functions).sort(), [
    "uniffi_a_fn_func_hello",
    "uniffi_b_fn_func_hello",
    "uniffi_c_fn_func_hello",
  ]);
});

test("a shared name with the same definition appears once", (t) => {
  const merged = uniffiMergeDefinitions([table("a"), table("b")]);
  t.assertEqual(Object.keys(merged.callbacks), [
    "ForeignFutureDroppedCallback",
  ]);
  t.assertEqual(Object.keys(merged.structs), [
    "ForeignFutureDroppedCallbackStruct",
  ]);
});

test("a shared callback name with a different definition throws", (t) => {
  const b = table("b");
  b.callbacks.ForeignFutureDroppedCallback = {
    args: [FfiType.UInt32],
    ret: FfiType.Void,
    hasRustCallStatus: false,
  };
  t.assertThrows(isMergeError("ForeignFutureDroppedCallback"), () =>
    uniffiMergeDefinitions([table("a"), b]),
  );
});

test("a shared struct name with a different definition throws", (t) => {
  const b = table("b");
  b.structs.ForeignFutureDroppedCallbackStruct = [
    { name: "handle", type: FfiType.UInt64 },
  ];
  t.assertThrows(isMergeError("ForeignFutureDroppedCallbackStruct"), () =>
    uniffiMergeDefinitions([table("a"), b]),
  );
});

test("names an object inherits are not mistaken for a collision", (t) => {
  const name: string = "constructor";
  const a = table("a");
  a.callbacks[name] = { args: [], ret: FfiType.Void, hasRustCallStatus: false };
  const merged = uniffiMergeDefinitions([a, table("b")]);
  t.assertEqual(merged.callbacks[name], a.callbacks[name]);
});

test("the merged table takes the first namespace's buffer symbols", (t) => {
  const merged = uniffiMergeDefinitions([table("a"), table("b")]);
  t.assertEqual(merged.symbols, table("a").symbols);
});

test("merging no tables throws", (t) => {
  t.assertThrows(
    (e: any) => e instanceof Error,
    () => uniffiMergeDefinitions([]),
  );
});
