/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import {
  UniffiNativeModule,
  FfiType,
  resolveLibPath,
  ResolveLibPathError,
} from "../lib.js";

test("ESM sees every named export", () => {
  assert.strictEqual(typeof UniffiNativeModule.open, "function");
  assert.deepStrictEqual(FfiType.Callback("cb"), {
    tag: "Callback",
    name: "cb",
  });
  assert.strictEqual(typeof resolveLibPath, "function");
  assert.ok(ResolveLibPathError.prototype instanceof Error);
});
