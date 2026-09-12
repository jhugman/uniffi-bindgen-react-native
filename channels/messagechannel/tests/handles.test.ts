/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { tagHandle, untagHandle } from "../src/handles.js";

test("port 0 tagging is the identity", () => {
  assert.strictEqual(tagHandle(7n, 0), 7n);
  assert.strictEqual(untagHandle(7n, 0), 7n);
});

test("port N sets bits 48+ and untag strips them", () => {
  const tagged = tagHandle(7n, 3);
  assert.strictEqual(tagged, 7n | (3n << 48n));
  assert.strictEqual(untagHandle(tagged, 3), 7n);
});

test("untag rejects a handle carrying another port's tag", () => {
  assert.throws(
    () => untagHandle(7n | (2n << 48n), 0),
    /tagged for port 2, expected port 0/,
  );
});

test("tag rejects a handle that already uses the tag bits", () => {
  assert.throws(() => tagHandle(1n << 50n, 0), /bits above 47/);
});
