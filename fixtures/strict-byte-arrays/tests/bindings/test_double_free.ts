/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Safety edges of the adopt-on-lower fix.
//
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- test_double_free
//
// When a library-owned `rustbuffer_alloc` view is lowered as an FFI argument it
// is *adopted*: the existing allocation is handed to the callee (which frees it)
// and the view's capacity hint is reset to 0. These tests pin the two guards
// that keep that safe — the shared-fixture analog of the napi runtime's
// "freeing an already-adopted argument view does not double free" test.
//
// We drive the raw scaffolding function directly (rather than the generated
// wrapper, which lowers a fresh view of its own) so the view we adopt is one we
// control. `measure_bytes(Vec<u8>) -> u32` is non-throwing, so the only
// exception that can surface is the runtime's own "already consumed" guard.
import "@/generated/uniffi_strict_byte_arrays";
import getNativeModule from "@/generated/uniffi_strict_byte_arrays-ffi";
import { test } from "@/asserts";
import "@/polyfills";

// Portable native-module handle: every flavor default-exports its
// `nativeModule()` getter from the `-ffi` module (JSI resolves to the global
// host object, napi/wasm2 to their registered module).
const nm: any = getNativeModule();

// The raw scaffolding symbol for `measure_bytes` is spelled differently per
// runtime — JSI prefixes host functions with `ubrn_`, napi/wasm2 use the bare
// FFI name — so probe for whichever the native module exposes.
const MEASURE_BYTES = [
  "ubrn_uniffi_uniffi_strict_byte_arrays_fn_func_measure_bytes",
  "uniffi_uniffi_strict_byte_arrays_fn_func_measure_bytes",
].find((name) => typeof nm[name] === "function");

// A library-owned view whose bytes are a valid serialized empty `Vec<u8>` (a
// 4-byte length prefix of 0), so `measure_bytes` lifts it cleanly rather than
// reading uninitialized memory as a length.
function allocEmptyVecView(): Uint8Array {
  const view = nm.rustbuffer_alloc(4);
  view[0] = 0;
  view[1] = 0;
  view[2] = 0;
  view[3] = 0;
  return view;
}

// Call the raw scaffolding function with a pre-lowered view, so *this* view is
// the one adopted (the generated wrapper would allocate and lower its own).
function callMeasure(view: Uint8Array): number {
  const status = { code: 0 };
  const result = nm[MEASURE_BYTES!](view, status);
  if (status.code !== 0) {
    throw new Error(`measure_bytes failed with status code ${status.code}`);
  }
  return result;
}

test("the raw measure_bytes scaffolding function is reachable", (t) => {
  t.assertNotNull(
    MEASURE_BYTES,
    "could not find measure_bytes on the native module",
  );
});

test("adopted argument view: a later rustbuffer_free is a no-op, not a double free", (t) => {
  const view = allocEmptyVecView();

  // Passing the view adopts it: the callee frees the allocation and the hint is
  // reset to 0.
  t.assertEqual(callMeasure(view), 0);

  // A defensive free of the now-adopted view must be a no-op. If the guard were
  // missing this would free already-freed memory and abort the process; reaching
  // the next line at all is the assertion.
  nm.rustbuffer_free(view);
  t.assertTrue(true, "rustbuffer_free on an adopted view did not crash");
});

test("adopted argument view: reusing it as an FFI argument is refused", (t) => {
  const view = allocEmptyVecView();

  // First use adopts the view and frees its backing allocation.
  callMeasure(view);

  // A second lowering must be refused rather than handing the callee a dangling
  // pointer into freed memory.
  t.assertThrows(
    (e: any) => String(e?.message ?? e).includes("already consumed"),
    () => callMeasure(view),
  );
});
