/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-docstrings -- jsi
//   cargo test -p uniffi-fixture-docstrings -- wasm
//   cargo test -p uniffi-fixture-docstrings -- napi
//
// The docstrings in `../../src/lib.rs` contain `*/`, which would close the
// generated JSDoc comment early if it were not escaped.
// TypeScript does not allow nested block comments. A regression fails
// during generation, before these assertions run, so the test below only
// needs to load the module and make a call.

import {
  identity,
  identityRecord,
  DocumentedEnum,
  DocumentedRecord,
} from "@/generated/uniffi_docstrings";
import { test } from "@/asserts";
import "@/polyfills";

test("module with block comments in docstrings is importable", (t) => {
  t.assertEqual("hello", identity("hello"));
  const record = identityRecord(DocumentedRecord.create({ name: "hello" }));
  t.assertEqual("hello", record.name);
  t.assertTrue(DocumentedEnum.First !== undefined);
});