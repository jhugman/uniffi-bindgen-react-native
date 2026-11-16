/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// When lib.js loads @ubjs/core, and what it says when the one it finds is too old.
//
// The tests run in order: the second needs the first to leave @ubjs/core unloaded.
import { test } from "node:test";
import assert from "node:assert";
import { createRequire, Module } from "node:module";
import lib from "../lib.js";
const { UniffiNativeModule } = lib;
import { libPath } from "./helpers/lib-path.mjs";
import {
  asyncDefinitions,
  legacyDefinitions,
} from "./helpers/test-lib-futures.mjs";

const require = createRequire(import.meta.url);
const corePath = require.resolve("@ubjs/core");
const { version } = require("../package.json");

const register = (definitions) =>
  UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
    definitions,
  );

test("a table without async entries does not load @ubjs/core", () => {
  register(legacyDefinitions());
  assert.strictEqual(require.cache[corePath], undefined);
});

test("an @ubjs/core without uniffiAsyncEntry fails the async register, naming the version it needs", () => {
  const tooOld = new Module(corePath);
  tooOld.filename = corePath;
  tooOld.loaded = true;
  tooOld.exports = {};
  require.cache[corePath] = tooOld;
  try {
    assert.throws(
      () => register(asyncDefinitions()),
      (e) => e.message.includes(`@ubjs/core ${version}`),
    );
  } finally {
    delete require.cache[corePath];
  }
});
