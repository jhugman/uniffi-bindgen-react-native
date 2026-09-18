/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Runs on Wasm2 (direct) and AsyncWasm (over a Worker). Every call is
// awaited: on Wasm2 the sync exports return values, on AsyncWasm promises,
// and `await` accepts both. The delta between the two runs is the channel.
// Every timed call also goes through one extra closure and promise in
// shape(), which is an equal floor under every µs/call figure on both
// flavors.
//
//   cargo test -p uniffi-fixture-benchmark -- wasm2::test_benchmark_async
//   cargo test -p uniffi-fixture-benchmark -- async_wasm::

import {
  addU32,
  buildTree,
  burn,
  callPing,
  type Counter,
  countLeaves,
  getBytes,
  getBytesAsync,
  getLargeRecord,
  getString,
  getStringArray,
  getStringAsync,
  makeCounter,
  noop,
  noopAsync,
  type Ping,
  takeBytes,
  takeBytesAsync,
  takeLargeRecord,
  takeString,
  takeStringArray,
  takeStringAsync,
} from "@/generated/uniffi_benchmark";
import { asyncTest } from "@/asserts";
import { benchAsync, fmtMs, RUNS, TIMEOUT_MS } from "./bench_helpers";

const SIZES: Array<{ label: string; bytes: number }> = [
  { label: "1 KB", bytes: 1_024 },
  { label: "64 KB", bytes: 64 * 1_024 },
  { label: "1 MB", bytes: 1_024 * 1_024 },
];

function perCall(label: string, ms: number, iters: number) {
  console.log(
    `  ${label.padEnd(28)} x${String(iters).padStart(6)}: ${fmtMs(ms).padStart(8)}ms  (~${((ms * 1000) / iters).toFixed(2).padStart(9)} µs/call)`,
  );
}

async function shape(
  label: string,
  iters: number,
  fn: () => Promise<unknown> | unknown,
) {
  const ms = await benchAsync(async () => fn(), iters, RUNS);
  perCall(label, ms, iters);
}

class TsPing implements Ping {
  async ping(n: number): Promise<number> {
    return n;
  }
}

/** Grow `iterations` until one `burn` takes about `targetMs`. */
async function calibrateBurn(targetMs: number): Promise<bigint> {
  let n = 1_000_000n;
  for (;;) {
    const t0 = performance.now();
    await burn(n);
    const ms = performance.now() - t0;
    if (ms >= targetMs) {
      return (n * BigInt(Math.round(targetMs))) / BigInt(Math.round(ms));
    }
    n *= 4n;
  }
}

/**
 * Run `fn` while a 1 ms interval ticks on the main thread. The longest gap
 * between ticks is how long the loop was blocked; the tick count against
 * the wall time is how much of the run the loop was free.
 */
async function observe(label: string, fn: () => Promise<unknown>) {
  let ticks = 0;
  let last = performance.now();
  let maxGap = 0;
  const tick = setInterval(() => {
    const now = performance.now();
    maxGap = Math.max(maxGap, now - last);
    last = now;
    ticks++;
  }, 1);
  const t0 = performance.now();
  let wall = 0;
  try {
    await fn();
    wall = performance.now() - t0;
    // One loop turn so the tick delayed by fn() lands before we read.
    await new Promise((r) => setTimeout(r, 0));
  } finally {
    // A failing assertion in fn() must not leave this interval running.
    clearInterval(tick);
  }
  // node's 1 ms interval delivers fewer than 1000 ticks per idle second, so
  // ticks/wall reads low even when the loop is free.
  console.log(
    `  ${label.padEnd(22)} wall=${fmtMs(wall).padStart(7)}ms  maxGap=${fmtMs(maxGap).padStart(7)}ms  ticks=${String(ticks).padStart(5)}/${Math.floor(wall)}`,
  );
}

(async () => {
  await asyncTest(
    "startup",
    async (t) => {
      const ms = (globalThis as any).__ubrnStartupMs;
      console.log(
        `\n--- startup (open wasm + initialize; AsyncWasm includes Worker spawn) ---\n  ${ms === undefined ? "n/a" : fmtMs(ms) + "ms"}`,
      );
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: scalar shapes",
    async (t) => {
      console.log("\n--- scalar shapes ---");
      await shape("noop", 10_000, () => noop());
      await shape("noopAsync", 10_000, () => noopAsync());
      await shape("addU32", 10_000, () => addU32(1, 2));
      // makeCounter() is typed CounterLike (interface); uniffiDestroy is
      // only on the concrete class, so narrow it for the cleanup call below.
      const c = (await makeCounter()) as Counter;
      await shape("Counter.increment", 10_000, () => c.increment());
      c.uniffiDestroy();
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: bytes and strings by size",
    async (t) => {
      console.log("\n--- bytes (transfer) and strings (clone) ---");
      for (const { label, bytes } of SIZES) {
        const iters = bytes >= 1_024 * 1_024 ? 100 : 1_000;
        const s = "x".repeat(bytes);
        const b = new ArrayBuffer(bytes);
        await shape(`getBytes ${label}`, iters, () => getBytes(bytes));
        await shape(`takeBytes ${label}`, iters, () => takeBytes(b));
        await shape(`getString ${label}`, iters, () => getString(bytes));
        await shape(`takeString ${label}`, iters, () => takeString(s));
      }
      const mb = 1_024 * 1_024;
      const s = "x".repeat(mb);
      const b1mb = new ArrayBuffer(mb);
      await shape("getBytesAsync 1 MB", 100, () => getBytesAsync(mb));
      await shape("takeBytesAsync 1 MB", 100, () => takeBytesAsync(b1mb));
      await shape("getStringAsync 1 MB", 100, () => getStringAsync(mb));
      await shape("takeStringAsync 1 MB", 100, () => takeStringAsync(s));
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: records, trees and string arrays",
    async (t) => {
      console.log("\n--- records, trees, string arrays ---");
      await shape("getLargeRecord", 1_000, () => getLargeRecord());
      const rec = await getLargeRecord();
      await shape("takeLargeRecord", 1_000, () => takeLargeRecord(rec));
      await shape("buildTree depth 8", 100, () => buildTree(8));
      const tree = await buildTree(8);
      await shape("countLeaves depth 8", 100, () => countLeaves(tree));
      const elem = "x".repeat(1_024);
      const arr = await getStringArray(1_024, elem);
      await shape("getStringArray 1024x1KB", 100, () =>
        getStringArray(1_024, elem),
      );
      await shape("takeStringArray 1024x1KB", 100, () => takeStringArray(arr));
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: callback round trips",
    async (t) => {
      console.log("\n--- callPing: Rust -> TS -> Rust per ping ---");
      const cb = new TsPing();
      t.assertEqual(await callPing(cb, 100), 4950);
      await shape("callPing x1", 1_000, () => callPing(cb, 1));
      await shape("callPing x100", 20, () => callPing(cb, 100));
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "responsiveness: main thread while Rust burns",
    async (t) => {
      console.log(
        "\n--- responsiveness: main-thread tick gaps during burn ---",
      );
      const n = await calibrateBurn(100);
      const expected = await burn(n);
      console.log(`  calibrated: burn(${n}) ~ 100ms`);
      await observe("one 100ms call", async () => {
        t.assertEqual(await burn(n), expected);
      });
      await observe("5 sequential calls", async () => {
        for (let i = 0; i < 5; i++) t.assertEqual(await burn(n), expected);
      });
      await observe("5 concurrent calls", async () => {
        const results = await Promise.all(
          Array.from({ length: 5 }, () => burn(n)),
        );
        for (const r of results) t.assertEqual(r, expected);
      });
      t.end();
    },
    TIMEOUT_MS,
  );
})();
