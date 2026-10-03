/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { FfiType } from "../src/ffi-type.js";
import { FfiType as CoreFfiType, type ModuleDefinitions } from "@ubjs/core";

test("scalar tags are stable references", () => {
  assert.strictEqual(FfiType.UInt32.tag, "UInt32");
  assert.strictEqual(FfiType.UInt32, FfiType.UInt32);
});

test("Callback builder yields a tagged object with a name", () => {
  const cb = FfiType.Callback("vt_calc_add");
  assert.deepStrictEqual(cb, { tag: "Callback", name: "vt_calc_add" });
});

test("Reference / MutReference / Struct builders compose", () => {
  assert.deepStrictEqual(FfiType.Struct("VTable_X"), {
    tag: "Struct",
    name: "VTable_X",
  });
  assert.deepStrictEqual(FfiType.Reference(FfiType.UInt32), {
    tag: "Reference",
    inner: FfiType.UInt32,
  });
  assert.deepStrictEqual(FfiType.MutReference(FfiType.UInt8), {
    tag: "MutReference",
    inner: FfiType.UInt8,
  });
});

test("FfiType and the table types come from @ubjs/core", () => {
  // Same object: the wasm runtime re-exports, it does not redefine.
  assert.strictEqual(FfiType, CoreFfiType);
  const defs: ModuleDefinitions = {
    symbols: {
      rustbuffer_alloc: "a",
      rustbuffer_free: "f",
      rustbuffer_from_bytes: "b",
    },
    functions: {},
    callbacks: {},
    structs: {},
  };
  assert.ok(defs);
});
