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
// Both directions are covered:
//
//   * `measureBytes` takes a byte array (lowered via `rustbuffer_alloc`) and
//     returns a scalar, so only the *argument* direction allocates a big buffer.
//   * `consumeProducedBytes` calls back into a foreign `BytesProducer` whose
//     `produce` returns a byte array, so the foreign side lowers and Rust consumes
//     what comes back. This is the *callback-return* direction, and it had no
//     coverage in any flavour until now — a runtime could copy instead of adopt
//     there and still pass a green suite, which is exactly what napi did. The fixture's
// counting allocator (see src/lib.rs) tracks live allocations >= 64 KiB, and we
// assert the live count is unchanged across a loop of calls. If the runtime
// copies the lowered view instead of adopting it, the allocation from
// `rustbuffer_alloc` is orphaned — codegen never frees a lowered argument — and
// the count climbs by exactly one per call.
import myModule, {
  measureBytes,
  consumeProducedBytes,
  liveBigAllocCount,
  liveBigAllocBytes,
  type BytesProducer,
} from "@/generated/uniffi_strict_byte_arrays";
import { test } from "@/asserts";

// The fixture now declares a callback interface, so its vtable has to be registered
// before Rust can call back into JS.
myModule.initialize();
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

test("buffer returned from a foreign callback is not leaked", (t) => {
  // The foreign side lowers `produce`'s return value through `rustbuffer_alloc`,
  // and Rust owns what comes back. If the runtime copies that view instead of
  // adopting it, the original allocation is orphaned — nothing frees a lowered
  // return value — and the live count climbs by exactly one per call.
  const producer: BytesProducer = {
    produce: (size: number) => new Uint8Array(size),
  };

  // Warm up so one-time allocations are inside the `before` snapshot.
  consumeProducedBytes(producer, PAYLOAD);

  const beforeCount = liveBigAllocCount();
  const beforeBytes = Number(liveBigAllocBytes());

  for (let i = 0; i < ITERATIONS; i++) {
    const len = consumeProducedBytes(producer, PAYLOAD);
    t.assertEqual(len, PAYLOAD);
  }

  const afterCount = liveBigAllocCount();
  const afterBytes = Number(liveBigAllocBytes());

  t.assertEqual(
    beforeCount,
    afterCount,
    `leaked ${afterCount - beforeCount} big buffers over ${ITERATIONS} callback returns`,
  );
  t.assertEqual(
    beforeBytes,
    afterBytes,
    `leaked ${afterBytes - beforeBytes} bytes over ${ITERATIONS} callback returns`,
  );
});
