/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/** A connection id, as the page makes them. */
export const ID = "0123456789abcdef0123456789abcdef";

/** Lets pending callbacks and fake port deliveries run. */
export const tick = () => new Promise((r) => setImmediate(r));

// Nothing here holds the event loop open, so a call that never settles would
// drain the loop and cancel every later test; this timer keeps each failure its own.
export function within<T>(promise: Promise<T>, ms = 1000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`not settled within ${ms} ms`)),
      ms,
    );
  });
  return Promise.race([promise, late]).finally(() => clearTimeout(timer));
}
