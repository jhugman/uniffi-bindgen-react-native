/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Over the channel, `measureBytes` copies its argument into the Worker; the
// live big-allocation count must still be flat across calls.
//
// To run:
//   cargo test -p uniffi-fixture-strict-byte-arrays -- async_wasm::test_leak
import {
  measureBytes,
  liveBigAllocCount,
  liveBigAllocBytes,
} from "@/generated/uniffi_strict_byte_arrays";
import { asyncTest } from "@/asserts";
import "@/polyfills";

// 256 KiB is above the allocator's 64 KiB threshold, so equality is exact.
const PAYLOAD = 256 * 1024;
const ITERATIONS = 64;

(async () => {
  await asyncTest(
    "lowered RustBuffer argument is not leaked across FFI calls",
    async (t) => {
      // Warm up once so any one-time big allocations are in the `before` snapshot.
      await measureBytes(new Uint8Array(PAYLOAD));

      const beforeCount = await liveBigAllocCount();
      const beforeBytes = Number(await liveBigAllocBytes());

      for (let i = 0; i < ITERATIONS; i++) {
        const view = new Uint8Array(PAYLOAD);
        const len = await measureBytes(view);
        t.assertEqual(len, PAYLOAD);
      }

      const afterCount = await liveBigAllocCount();
      const afterBytes = Number(await liveBigAllocBytes());

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
      t.end();
    },
  );
})();
