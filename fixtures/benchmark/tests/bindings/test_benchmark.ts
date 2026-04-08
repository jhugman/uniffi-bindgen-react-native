/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-benchmark -- jsi
//   cargo test -p uniffi-fixture-benchmark -- wasm

import {
  getBytes,
  getString,
  getStringArray,
  takeBytes,
  takeString,
  takeStringArray,
} from "@/generated/uniffi_benchmark";
import { test } from "@/asserts";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Run `fn` `iterations` times and return wall-clock ms. */
function timeMs(fn: () => void, iterations: number): number {
  const start = Date.now();
  for (let i = 0; i < iterations; i++) fn();
  return Date.now() - start;
}

/** Return the minimum over `runs` calls of timeMs(fn, iterations). */
function bench(fn: () => void, iterations: number, runs: number): number {
  let best = Infinity;
  for (let r = 0; r < runs; r++) {
    best = Math.min(best, timeMs(fn, iterations));
  }
  return best;
}

const RUNS = 3;

// Payload sizes for individual string / byte benchmarks.
const SIZES: Array<{ label: string; bytes: number }> = [
  { label: "1 KB", bytes: 1_024 },
  { label: "16 KB", bytes: 16 * 1_024 },
  { label: "256 KB", bytes: 256 * 1_024 },
  { label: "512 KB", bytes: 512 * 1_024 },
  { label: "1 MB", bytes: 1_024 * 1_024 },
];

// Configurations for array benchmarks: [count, element_bytes].
const ARRAY_CONFIGS: Array<{ count: number; elemBytes: number }> = [
  { count: 100, elemBytes: 64 },
  { count: 1000, elemBytes: 64 },
  { count: 100, elemBytes: 1024 },
  { count: 1024, elemBytes: 100 },
];

// ---------------------------------------------------------------------------
// String benchmarks
// ---------------------------------------------------------------------------

test("bench: getString (lifting a string from Rust)", (_t) => {
  console.log("\n--- getString: lifting a string from Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES) {
    const ms = bench(() => getString(bytes), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

test("bench: takeString (lowering a string into Rust)", (_t) => {
  console.log("\n--- takeString: lowering a string into Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES) {
    const s = "x".repeat(bytes);
    const ms = bench(() => takeString(s), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// Bytes benchmarks
// ---------------------------------------------------------------------------

test("bench: getBytes (lifting bytes from Rust)", (_t) => {
  console.log("\n--- getBytes: lifting bytes from Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES) {
    const ms = bench(() => getBytes(bytes), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

test("bench: takeBytes (lowering bytes into Rust)", (_t) => {
  console.log("\n--- takeBytes: lowering bytes into Rust ---");
  const ITERS = 100;
  for (const { label, bytes } of SIZES) {
    // const b = new Uint8Array(bytes);
    const b = new ArrayBuffer(bytes);
    const ms = bench(() => takeBytes(b), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(7)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

// ---------------------------------------------------------------------------
// String-array benchmarks
// ---------------------------------------------------------------------------

test("bench: getStringArray (lifting a Vec<String> from Rust)", (_t) => {
  console.log("\n--- getStringArray: lifting a Vec<String> from Rust ---");
  const ITERS = 20;
  for (const { count, elemBytes } of ARRAY_CONFIGS) {
    const label = `${count}×${elemBytes}B`;
    const s = "x".repeat(elemBytes);
    const ms = bench(() => getStringArray(count, s), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(12)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});

test("bench: takeStringArray (lowering a string[] into Rust)", (_t) => {
  console.log("\n--- takeStringArray: lowering a string[] into Rust ---");
  const ITERS = 20;
  for (const { count, elemBytes } of ARRAY_CONFIGS) {
    const label = `${count}×${elemBytes}B`;
    const arr = Array.from({ length: count }, () => "x".repeat(elemBytes));
    const ms = bench(() => takeStringArray(arr), ITERS, RUNS);
    console.log(
      `  ${label.padEnd(12)} x${ITERS}: ${ms}ms  (~${(ms / ITERS).toFixed(2)} ms/call)`,
    );
  }
});
