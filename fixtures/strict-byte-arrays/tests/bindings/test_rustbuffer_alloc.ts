/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- jsi
//
// Exercises the JSI codegen-emitted host functions
// `<ModuleName>.rustbuffer_alloc(n)` and `<ModuleName>.rustbuffer_free(view)`.

// Side-effect import to ensure registerNatives has installed the host object
// onto globalThis before we look it up.
import "@/generated/uniffi_strict_byte_arrays";
import { test } from "@/asserts";
import "@/polyfills";

const nm = (globalThis as any).NativeUniffiStrictByteArrays;

test("rustbuffer_alloc returns a Uint8Array of the requested length", (t) => {
  const view = nm.rustbuffer_alloc(16);
  t.assertTrue(view instanceof Uint8Array, "view is a Uint8Array");
  t.assertEqual(view.byteLength, 16);
  t.assertEqual(view.byteOffset, 0);
  // Bytes survive write/read on the JS side.
  view[0] = 0xab;
  view[15] = 0xcd;
  t.assertEqual(view[0], 0xab);
  t.assertEqual(view[15], 0xcd);
  // Hand the buffer back to Rust to free it (otherwise it leaks).
  nm.rustbuffer_free(view);
});

test("rustbuffer_alloc handles zero-length allocation", (t) => {
  const view = nm.rustbuffer_alloc(0);
  t.assertTrue(view instanceof Uint8Array, "view is a Uint8Array");
  t.assertEqual(view.byteLength, 0);
  nm.rustbuffer_free(view);
});
