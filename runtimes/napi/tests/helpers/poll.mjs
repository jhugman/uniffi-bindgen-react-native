/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Polling helper for cross-thread tests.
//
// Uses setTimeout(fn, 0) instead of setImmediate so that napi-rs
// ThreadsafeFunction callbacks (delivered via uv_async in libuv's poll
// phase) get a chance to run between poll iterations. setImmediate runs
// in the check phase and can starve the poll phase on Linux.

export function pollUntil(
  checkFn,
  timeoutMsg = "Timed out polling",
  maxAttempts = 100,
) {
  return new Promise((resolve, reject) => {
    let attempts = 0;
    const poll = () => {
      attempts++;
      if (checkFn()) resolve();
      else if (attempts > maxAttempts) reject(new Error(timeoutMsg));
      else setTimeout(poll, 0);
    };
    setTimeout(poll, 0);
  });
}
