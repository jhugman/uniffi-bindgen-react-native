/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Proves a buffer returned from a foreign callback is adopted, not copied and orphaned.
//
// To run:
//   cargo test -p uniffi-fixture-sync-callbacks -- jsi::test_sync_callbacks_leak
//
// The fixture's counting allocator (see src/bytes.rs) tracks live allocations >= 64 KiB.
import myModule, {
  consumeProducedBytes,
  liveBigAllocCount,
  liveBigAllocBytes,
  type BytesProducer,
} from "@/generated/sync_callbacks";
import { test } from "@/asserts";

// The vtable has to be registered before Rust can call back into JS.
myModule.initialize();
import "@/polyfills";

// 256 KiB is comfortably above the allocator's 64 KiB threshold, so a leak is
// unmistakable and the assertion is exact equality rather than a threshold.
const PAYLOAD = 256 * 1024;
const ITERATIONS = 64;

test("buffer returned from a foreign callback is not leaked", (t) => {
  // The foreign side lowers `produce`'s return value through `rustbuffer_alloc`,
  // and Rust owns what comes back. If the runtime copies that view instead of
  // adopting it, the original allocation is orphaned — nothing frees a lowered
  // return value — and the live count climbs by exactly one per call.
  const producer: BytesProducer = {
    produce: (size: number) => new ArrayBuffer(size),
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
