/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-benchmark -- jsi
//   cargo test -p uniffi-fixture-benchmark -- wasm

import {
  addU32,
  addU32Async,
  buildTree,
  Counter,
  countLeaves,
  getBytes,
  getBytesAsync,
  getLargeRecord,
  getString,
  getStringArray,
  getStringArrayAsync,
  getStringAsync,
  noop,
  noopAsync,
  takeBytes,
  takeBytesAsync,
  takeLargeRecord,
  takeString,
  takeStringArray,
  takeStringArrayAsync,
  takeStringAsync,
} from "@/generated/uniffi_benchmark";
import { asyncTest, test } from "@/asserts";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Run `fn` `iterations` times and return wall-clock ms (sub-ms resolution). */
function timeMs(fn: () => void, iterations: number): number {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) fn();
  return performance.now() - start;
}

/** Return the minimum over `runs` calls of timeMs(fn, iterations). */
function bench(fn: () => void, iterations: number, runs: number): number {
  let best = Infinity;
  for (let r = 0; r < runs; r++) {
    best = Math.min(best, timeMs(fn, iterations));
  }
  return best;
}

/** Async variant: awaits each call sequentially. */
async function timeMsAsync(
  fn: () => Promise<unknown>,
  iterations: number,
): Promise<number> {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) await fn();
  return performance.now() - start;
}

async function benchAsync(
  fn: () => Promise<unknown>,
  iterations: number,
  runs: number,
): Promise<number> {
  let best = Infinity;
  for (let r = 0; r < runs; r++) {
    best = Math.min(best, await timeMsAsync(fn, iterations));
  }
  return best;
}

/** Format a sub-ms time with reasonable precision. */
function fmtMs(ms: number): string {
  if (ms >= 100) return ms.toFixed(0);
  if (ms >= 10) return ms.toFixed(1);
  if (ms >= 1) return ms.toFixed(2);
  return ms.toFixed(3);
}

const RUNS = 3;

// Sizes for the focused large-transfer suite (sync + async).
const SIZES_LARGE: Array<{ label: string; bytes: number }> = [
  { label: "512 KB", bytes: 512 * 1_024 },
  { label: "1 MB", bytes: 1_024 * 1_024 },
];

// String-array configurations that each total 1 MB of string payload.
// Trades array length for element length to see how per-element overhead
// vs total payload size dominates.
const ARRAY_1MB_CONFIGS: Array<{ count: number; elemBytes: number }> = [
  { count: 1, elemBytes: 1_048_576 },
  { count: 64, elemBytes: 16_384 },
  { count: 1024, elemBytes: 1_024 },
  { count: 16_384, elemBytes: 64 },
  { count: 65_536, elemBytes: 16 },
];

const TIMEOUT_MS = 1_000_000; // 1000s = 16.7m, which is long but allows debugging in CI if needed

// ---------------------------------------------------------------------------
// String benchmarks
// ---------------------------------------------------------------------------

test("bench: getString (lifting a string from Rust)", (_t) => {
  console.log("\n--- getString: lifting a string from Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES_LARGE) {
    const ms = bench(() => getString(bytes), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

test("bench: takeString (lowering a string into Rust)", (_t) => {
  console.log("\n--- takeString: lowering a string into Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES_LARGE) {
    const s = "x".repeat(bytes);
    const ms = bench(() => takeString(s), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// Bytes benchmarks
// ---------------------------------------------------------------------------

test("bench: getBytes (lifting bytes from Rust)", (_t) => {
  console.log("\n--- getBytes: lifting bytes from Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES_LARGE) {
    const ms = bench(() => getBytes(bytes), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

test("bench: takeBytes (lowering bytes into Rust)", (_t) => {
  console.log("\n--- takeBytes: lowering bytes into Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES_LARGE) {
    // const b = new Uint8Array(bytes);
    const b = new ArrayBuffer(bytes);
    const ms = bench(() => takeBytes(b), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// FFI overhead micro suite
// ---------------------------------------------------------------------------

test("bench: noop (zero-payload call overhead)", (_t) => {
  console.log("\n--- noop: pure FFI crossing cost ---");
  const ITERS = 100_000;
  const ms = bench(() => noop(), ITERS, RUNS);
  console.log(
    `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
  );
});

test("bench: addU32 (two-scalar marshaling)", (_t) => {
  console.log("\n--- addU32: scalar marshaling, no buffer ---");
  const ITERS = 100_000;
  const ms = bench(() => addU32(1, 2), ITERS, RUNS);
  console.log(
    `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
  );
});

test("bench: Counter.increment (object handle + method dispatch)", (_t) => {
  console.log("\n--- Counter.increment: handle passing + vtable cost ---");
  const ITERS = 100_000;
  const c = new Counter();
  const ms = bench(() => c.increment(), ITERS, RUNS);
  console.log(
    `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
  );
});

// ---------------------------------------------------------------------------
// Wide record
// ---------------------------------------------------------------------------

test("bench: getLargeRecord (lifting a 20-field record)", (_t) => {
  console.log("\n--- getLargeRecord: wide-record lift ---");
  const ITERS = 10_000;
  const ms = bench(() => getLargeRecord(), ITERS, RUNS);
  console.log(
    `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
  );
});

test("bench: takeLargeRecord (lowering a 20-field record)", (_t) => {
  console.log("\n--- takeLargeRecord: wide-record lower ---");
  const ITERS = 10_000;
  const rec = getLargeRecord();
  const ms = bench(() => takeLargeRecord(rec), ITERS, RUNS);
  console.log(
    `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
  );
});

// ---------------------------------------------------------------------------
// Recursive enum (binary tree, vec-backed)
// ---------------------------------------------------------------------------

const TREE_DEPTHS = [4, 8, 12]; // 16 / 256 / 4096 leaves

test("bench: buildTree (lifting a recursive enum)", (_t) => {
  console.log("\n--- buildTree: recursive-enum lift ---");
  const ITERS = 100;
  for (const depth of TREE_DEPTHS) {
    const leaves = 1 << depth;
    const ms = bench(() => buildTree(depth), ITERS, RUNS);
    console.log(
      `  depth=${depth.toString().padEnd(2)} (${leaves} leaves) x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

test("bench: countLeaves (lowering a recursive enum)", (_t) => {
  console.log("\n--- countLeaves: recursive-enum lower ---");
  const ITERS = 100;
  for (const depth of TREE_DEPTHS) {
    const leaves = 1 << depth;
    const tree = buildTree(depth);
    const ms = bench(() => countLeaves(tree), ITERS, RUNS);
    console.log(
      `  depth=${depth.toString().padEnd(2)} (${leaves} leaves) x${ITERS}: ${fmtMs(ms)}ms  (~${(ms / ITERS).toFixed(3)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// Large-transfer suite (sync, 512K + 1MB only)
// ---------------------------------------------------------------------------

test("bench: large get/take (sync, 512K + 1MB)", (_t) => {
  console.log("\n--- large transfers (sync, 512K + 1MB) ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES_LARGE) {
    const s = "x".repeat(bytes);
    const b = new ArrayBuffer(bytes);

    const tGetS = bench(() => getString(bytes), ITERS, RUNS);
    const tTakeS = bench(() => takeString(s), ITERS, RUNS);
    const tGetB = bench(() => getBytes(bytes), ITERS, RUNS);
    const tTakeB = bench(() => takeBytes(b), ITERS, RUNS);

    console.log(
      `  ${label.padEnd(7)} x${ITERS}  ` +
        `getString=${fmtMs(tGetS).padStart(7)}ms  ` +
        `takeString=${fmtMs(tTakeS).padStart(7)}ms  ` +
        `getBytes=${fmtMs(tGetB).padStart(7)}ms  ` +
        `takeBytes=${fmtMs(tTakeB).padStart(7)}ms`,
    );
  }
});

// ---------------------------------------------------------------------------
// String-array — 1 MB total payload, varied (count × elem size)
// ---------------------------------------------------------------------------

test("bench: string-array 1MB total payload (sync)", (_t) => {
  console.log("\n--- string-array, 1MB total (count × elemBytes) sync ---");
  const ITERS = 10;
  for (const { count, elemBytes } of ARRAY_1MB_CONFIGS) {
    const label = `${String(count).padStart(6)}×${String(elemBytes).padEnd(7)}B`;
    const s = "x".repeat(elemBytes);
    const arr = Array.from({ length: count }, () => s);

    const tGet = bench(() => getStringArray(count, s), ITERS, RUNS);
    const tTake = bench(() => takeStringArray(arr), ITERS, RUNS);

    console.log(
      `  ${label}  x${ITERS}  ` +
        `get=${fmtMs(tGet).padStart(7)}ms (${(tGet / ITERS).toFixed(2)} ms/call)  ` +
        `take=${fmtMs(tTake).padStart(7)}ms (${(tTake / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// Memory probes
// ---------------------------------------------------------------------------

test("MEM: heap + wasm-memory profile", (_t) => {
  console.log("\n--- MEM: heap + wasm linear memory ---");
  const gc: (() => void) | undefined = (globalThis as any).gc;
  if (!gc) {
    console.log(
      "  (run with NODE_OPTIONS=--expose-gc for accurate steady-state numbers)",
    );
  }

  function settle() {
    if (gc) {
      gc();
      gc();
    }
  }
  const mb = (n: number) => `${(n / 1_048_576).toFixed(2)} MB`;

  // Node's `external` includes WebAssembly.Memory linear memory + ArrayBuffer
  // backing stores, so we can use it as a proxy for wasm + off-heap growth.
  function probe(label: string, iters: number, fn: () => void) {
    settle();
    const before = process.memoryUsage();
    // Allocation-rate run (NO GC mid-loop).
    for (let i = 0; i < iters; i++) fn();
    const peak = process.memoryUsage();
    settle();
    const after = process.memoryUsage();
    const allocPerCallKB = (peak.heapUsed - before.heapUsed) / iters / 1024;
    const extPerCallKB = (peak.external - before.external) / iters / 1024;
    console.log(
      `  ${label.padEnd(24)} N=${String(iters).padStart(7)}  ` +
        `heap Δalloc=${mb(peak.heapUsed - before.heapUsed).padStart(9)}  ` +
        `Δretained=${mb(after.heapUsed - before.heapUsed).padStart(9)}  ` +
        `~${allocPerCallKB.toFixed(2).padStart(7)} KB/call heap, ` +
        `${extPerCallKB.toFixed(2).padStart(6)} KB/call ext`,
    );
  }

  settle();
  const mu0 = process.memoryUsage();
  console.log(
    `  baseline:  rss=${mb(mu0.rss)}  heap=${mb(mu0.heapUsed)}/${mb(mu0.heapTotal)}  external=${mb(mu0.external)} (incl. wasm)`,
  );

  probe("getBytes(1MB)", 500, () => {
    const r = getBytes(1_024 * 1_024);
    if (r.byteLength === 0) throw new Error();
  });
  probe("takeBytes(1MB)", 500, () => {
    takeBytes(new ArrayBuffer(1_024 * 1_024));
  });
  probe("getString(1MB)", 500, () => {
    const s = getString(1_024 * 1_024);
    if (s.length === 0) throw new Error();
  });
  probe("buildTree(depth=12)", 200, () => {
    const t = buildTree(12);
    if (!t) throw new Error();
  });
  const t12 = buildTree(12);
  probe("countLeaves(depth=12)", 200, () => {
    if (countLeaves(t12) === 0) throw new Error();
  });
  probe("noop", 500_000, () => {
    noop();
  });

  settle();
  const mu1 = process.memoryUsage();
  console.log(
    `  final:     rss=${mb(mu1.rss)}  heap=${mb(mu1.heapUsed)}/${mb(mu1.heapTotal)}  external=${mb(mu1.external)} (incl. wasm)`,
  );
});

// ---------------------------------------------------------------------------
// Async suite — uses asyncTest inside an IIFE so each test awaits before the
// next prints. `test()` is sync-only; passing it an async function returns an
// un-awaited promise and the output interleaves.
// ---------------------------------------------------------------------------

(async () => {
  await asyncTest(
    "bench: noopAsync (async FFI crossing cost)",
    async (t) => {
      console.log("\n--- noopAsync: pure async FFI crossing cost ---");
      const ITERS = 10_000;
      const ms = await benchAsync(() => noopAsync(), ITERS, RUNS);
      console.log(
        `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
      );
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: addU32Async (async scalar marshaling)",
    async (t) => {
      console.log("\n--- addU32Async: scalar marshaling, async ---");
      const ITERS = 10_000;
      const ms = await benchAsync(() => addU32Async(1, 2), ITERS, RUNS);
      console.log(
        `  x${ITERS}: ${fmtMs(ms)}ms  (~${((ms * 1000) / ITERS).toFixed(3)} µs/call)`,
      );
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: large get/take async (512K + 1MB)",
    async (t) => {
      console.log("\n--- large transfers (async, 512K + 1MB) ---");
      const ITERS = 100;
      for (const { label, bytes } of SIZES_LARGE) {
        const s = "x".repeat(bytes);
        const b = new ArrayBuffer(bytes);

        const tGetS = await benchAsync(
          () => getStringAsync(bytes),
          ITERS,
          RUNS,
        );
        const tTakeS = await benchAsync(() => takeStringAsync(s), ITERS, RUNS);
        const tGetB = await benchAsync(() => getBytesAsync(bytes), ITERS, RUNS);
        const tTakeB = await benchAsync(() => takeBytesAsync(b), ITERS, RUNS);

        console.log(
          `  ${label.padEnd(7)} x${ITERS}  ` +
            `getString=${fmtMs(tGetS).padStart(7)}ms  ` +
            `takeString=${fmtMs(tTakeS).padStart(7)}ms  ` +
            `getBytes=${fmtMs(tGetB).padStart(7)}ms  ` +
            `takeBytes=${fmtMs(tTakeB).padStart(7)}ms`,
        );
      }
      t.end();
    },
    TIMEOUT_MS,
  );

  await asyncTest(
    "bench: string-array 1MB total payload (async)",
    async (t) => {
      console.log(
        "\n--- string-array, 1MB total (count × elemBytes) async ---",
      );
      const ITERS = 10;
      for (const { count, elemBytes } of ARRAY_1MB_CONFIGS) {
        const label = `${String(count).padStart(6)}×${String(elemBytes).padEnd(7)}B`;
        const s = "x".repeat(elemBytes);
        const arr = Array.from({ length: count }, () => s);

        const tGet = await benchAsync(
          () => getStringArrayAsync(count, s),
          ITERS,
          RUNS,
        );
        const tTake = await benchAsync(
          () => takeStringArrayAsync(arr),
          ITERS,
          RUNS,
        );

        console.log(
          `  ${label}  x${ITERS}  ` +
            `get=${fmtMs(tGet).padStart(7)}ms (${(tGet / ITERS).toFixed(2)} ms/call)  ` +
            `take=${fmtMs(tTake).padStart(7)}ms (${(tTake / ITERS).toFixed(2)} ms/call)`,
        );
      }
      t.end();
    },
    TIMEOUT_MS,
  );
})();
