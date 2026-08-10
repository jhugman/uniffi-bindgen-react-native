/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Proves whether lowering a `RustBuffer` argument leaks the library-owned
// allocation.
//
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- jsi::test_leak
//
// `measureBytes` takes a byte array (lowered via `rustbuffer_alloc`) and returns
// a scalar, so only the argument direction allocates a big buffer. The fixture's
// counting allocator (see src/lib.rs) tracks live allocations >= 64 KiB, and we
// assert the live count is unchanged across a loop of calls. If the runtime
// copies the lowered view instead of adopting it, the allocation from
// `rustbuffer_alloc` is orphaned — codegen never frees a lowered argument — and
// the count climbs by exactly one per call.
import {
  measureBytes,
  liveBigAllocCount,
  liveBigAllocBytes,
} from "@/generated/uniffi_strict_byte_arrays";
import { test } from "@/asserts";
import "@/polyfills";

// 256 KiB is comfortably above the allocator's 64 KiB threshold, so a leak is
// unmistakable and the assertion is exact equality rather than a threshold.
const PAYLOAD = 256 * 1024;
const ITERATIONS = 64;

test("lowered RustBuffer argument is not leaked across FFI calls", (t) => {
  // Warm up once so any one-time big allocations are already reflected in the
  // `before` snapshot and don't count as a leak.
  measureBytes(new Uint8Array(PAYLOAD));

  const beforeCount = liveBigAllocCount();
  const beforeBytes = Number(liveBigAllocBytes());

  for (let i = 0; i < ITERATIONS; i++) {
    const view = new Uint8Array(PAYLOAD);
    const len = measureBytes(view);
    t.assertEqual(len, PAYLOAD);
  }

  const afterCount = liveBigAllocCount();
  const afterBytes = Number(liveBigAllocBytes());

  t.assertEqual(
    beforeCount,
    afterCount,
    `leaked ${afterCount - beforeCount} big buffers over ${ITERATIONS} calls`,
  );
  t.assertEqual(
    beforeBytes,
    afterBytes,
    `leaked ${afterBytes - beforeBytes} bytes over ${ITERATIONS} calls`,
  );
});
