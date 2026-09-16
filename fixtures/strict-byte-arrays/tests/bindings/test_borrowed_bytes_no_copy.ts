/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Proves that a `[ByRef] bytes` (`&[u8]`) argument is really borrowed, not
// lowered through a `RustBuffer` copy.
//
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- napi::test_borrowed
//   cargo test -p uniffi-fixture-strict-byte-arrays -- jsi::test_borrowed    (needs Hermes)
//
// The roundtrip tests in `test_strict_byte_arrays.ts` only check content, so an
// implementation that lowered `&[u8]` through the ordinary `Vec<u8>` path (i.e.
// `rustbuffer_alloc` + copy) would pass every one of them. This test uses the
// fixture's counting global allocator (see `src/lib.rs`) instead: it snapshots
// the number of live Rust allocations >= 64 KiB immediately before and after a
// loop of borrowed-bytes calls. A borrowing lowering never calls
// `rustbuffer_alloc`, so the delta is zero; a copy-based lowering allocates one
// big buffer per call that nothing frees (the callee only borrows a
// `ForeignBytes`), so the delta climbs by exactly one per call.
//
// Why only [Jsi, Napi]: these are the only flavours that can pass the caller's
// buffer through by pointer. On both Wasm flavours the ABI *has* to materialise
// the argument in linear memory — classic `Wasm`'s wasm-bindgen boundary copies
// any `Uint8Array` argument into a module-owned `Vec<u8>` before the shim can
// borrow it, and `wasm2`'s player copies it through `__ubrn_alloc` — so the
// allocator moves for a borrow and for a copy alike, and the counter cannot
// attribute the movement to the lowering. That copy is the documented design
// there (one copy on the wasm flavours), so there is no "no copy" claim to test.
import {
  borrowedBytesChecksum,
  copyBorrowedBytes,
  measureBytes,
  liveBigAllocCount,
  liveBigAllocBytes,
} from "@/generated/uniffi_strict_byte_arrays";
import getNativeModule from "@/generated/uniffi_strict_byte_arrays-ffi";
import { test } from "@/asserts";
import "@/polyfills";

// Comfortably above the allocator's 64 KiB threshold, so a leaked buffer is
// unmistakable and the assertion can be exact equality rather than a threshold.
const PAYLOAD = 256 * 1024;
const ITERATIONS = 64;

function snapshot(): { count: number; bytes: number } {
  return { count: liveBigAllocCount(), bytes: Number(liveBigAllocBytes()) };
}

test("borrowed bytes arguments are not lowered through a RustBuffer copy", (t) => {
  // A non-empty payload, so a copy is real work and the checksum is a usable
  // content check (`1 * PAYLOAD` fits in the u32 the fixture returns).
  const view = new Uint8Array(PAYLOAD).fill(1);

  // Warm up once so any one-time big allocations land in the `before` snapshot.
  borrowedBytesChecksum(view);

  const beforeChecksum = snapshot();
  for (let i = 0; i < ITERATIONS; i++) {
    t.assertEqual(borrowedBytesChecksum(view), PAYLOAD);
  }
  const afterChecksum = snapshot();
  t.assertEqual(
    beforeChecksum.count,
    afterChecksum.count,
    `borrowedBytesChecksum leaked ${afterChecksum.count - beforeChecksum.count} big buffers over ${ITERATIONS} calls`,
  );
  t.assertEqual(
    beforeChecksum.bytes,
    afterChecksum.bytes,
    `borrowedBytesChecksum leaked ${afterChecksum.bytes - beforeChecksum.bytes} bytes over ${ITERATIONS} calls`,
  );

  // `copyBorrowedBytes` returns a `Vec<u8>`, but the *argument* direction must
  // still borrow: the returned buffer is lifted and freed by the wrapper, so the
  // live count has to settle back exactly where it started.
  copyBorrowedBytes(view);
  const beforeCopy = snapshot();
  for (let i = 0; i < ITERATIONS; i++) {
    // Compare lengths, not contents — stringifying a 256 KiB array on every
    // iteration would thrash the Hermes GC at MB scale.
    t.assertEqual(copyBorrowedBytes(view).byteLength, PAYLOAD);
  }
  const afterCopy = snapshot();
  t.assertEqual(
    beforeCopy.count,
    afterCopy.count,
    `copyBorrowedBytes leaked ${afterCopy.count - beforeCopy.count} big buffers over ${ITERATIONS} calls`,
  );
  t.assertEqual(
    beforeCopy.bytes,
    afterCopy.bytes,
    `copyBorrowedBytes leaked ${afterCopy.bytes - beforeCopy.bytes} bytes over ${ITERATIONS} calls`,
  );
});

test("control: the counter observes a live big RustBuffer allocation", (t) => {
  // The borrowed assertions above are only meaningful if the counter actually
  // moves for an allocation of this size on this path. `measureBytes`'s
  // `Vec<u8>` argument is lowered with `rustbuffer_alloc` — exactly the
  // allocation a copy-based `&[u8]` lowering would add — so allocate that
  // buffer by hand, hold it, and check the counter sees it before freeing it.
  const nm: any = getNativeModule();
  const before = snapshot();

  const buffer = nm.rustbuffer_alloc(PAYLOAD);
  const held = snapshot();
  t.assertEqual(
    held.count,
    before.count + 1,
    "counter did not observe the big RustBuffer the owned-bytes lowering allocates",
  );
  t.assertEqual(
    held.bytes,
    before.bytes + PAYLOAD,
    "counter saw the wrong byte count for the big RustBuffer",
  );

  nm.rustbuffer_free(buffer);
  const freed = snapshot();
  t.assertEqual(
    freed.count,
    before.count,
    "freeing the big RustBuffer did not settle the count",
  );
  t.assertEqual(
    freed.bytes,
    before.bytes,
    "freeing the big RustBuffer did not settle the byte count",
  );

  // And the owned-bytes entry point itself still round-trips the payload.
  t.assertEqual(measureBytes(new Uint8Array(PAYLOAD)), PAYLOAD);
});
