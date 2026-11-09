/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/** Run `fn` `iterations` times and return wall-clock ms (sub-ms resolution). */
export function timeMs(fn: () => void, iterations: number): number {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) fn();
  return performance.now() - start;
}

/** Return the minimum over `runs` calls of timeMs(fn, iterations). */
export function bench(
  fn: () => void,
  iterations: number,
  runs: number,
): number {
  let best = Infinity;
  for (let r = 0; r < runs; r++) {
    best = Math.min(best, timeMs(fn, iterations));
  }
  return best;
}

/** Async variant: awaits each call sequentially. */
export async function timeMsAsync(
  fn: () => Promise<unknown>,
  iterations: number,
): Promise<number> {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) await fn();
  return performance.now() - start;
}

export async function benchAsync(
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
export function fmtMs(ms: number): string {
  if (ms >= 100) return ms.toFixed(0);
  if (ms >= 10) return ms.toFixed(1);
  if (ms >= 1) return ms.toFixed(2);
  return ms.toFixed(3);
}

export const RUNS = 3;

// Sizes for the focused large-transfer suite (sync + async).
export const SIZES_LARGE: Array<{ label: string; bytes: number }> = [
  { label: "512 KB", bytes: 512 * 1_024 },
  { label: "1 MB", bytes: 1_024 * 1_024 },
];

// String-array configurations that each total 1 MB of string payload.
// Trades array length for element length to see how per-element overhead
// vs total payload size dominates.
export const ARRAY_1MB_CONFIGS: Array<{ count: number; elemBytes: number }> = [
  { count: 1, elemBytes: 1_048_576 },
  { count: 64, elemBytes: 16_384 },
  { count: 1024, elemBytes: 1_024 },
  { count: 16_384, elemBytes: 64 },
  { count: 65_536, elemBytes: 16 },
];

export const TIMEOUT_MS = 1_000_000; // 1000s = 16.7m, which is long but allows debugging in CI if needed
