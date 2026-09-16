/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "../testing/polyfills";
import { asyncTest } from "../testing/asserts";
import {
  UniffiRustCaller,
  uniffiRustCallAsync,
  type UniffiRustCallStatus,
  type UniffiHandle,
} from "../src/index";

const caller = new UniffiRustCaller<UniffiRustCallStatus>(() => ({ code: 0 }));
const liftString = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

(async () => {
  await asyncTest(
    "the poll loop awaits the future and complete calls",
    async (t) => {
      const calls: string[] = [];
      const result = await uniffiRustCallAsync(
        caller,
        /*rustFutureFunc:*/ async () => {
          calls.push("future");
          // ts target is es5 in the test harness, so use BigInt() instead of `n` literals.
          return BigInt(7);
        },
        /*pollFunc:*/ (rustFuture, cb, handle: UniffiHandle) => {
          calls.push(`poll ${rustFuture}`);
          // A player over a port fires the continuation later, from a message.
          setTimeout(() => cb(handle, 0 /* READY */), 0);
        },
        /*cancelFunc:*/ () => calls.push("cancel"),
        /*completeFunc:*/ async (rustFuture, status) => {
          calls.push(`complete ${rustFuture}`);
          status.code = 0;
          return 99;
        },
        /*freeFunc:*/ (rustFuture) => calls.push(`free ${rustFuture}`),
        /*liftFunc:*/ (n: number) => n + 1,
        liftString,
      );
      t.assertEqual(result, 100);
      t.assertEqual(calls.join(","), "future,poll 7,complete 7,free 7");
      t.end();
    },
  );

  await asyncTest(
    "an abort fired synchronously right after the call is not missed",
    async (t) => {
      const calls: string[] = [];
      const abortController = new AbortController();
      const promise = uniffiRustCallAsync(
        caller,
        /*rustFutureFunc:*/ async () => {
          calls.push("future");
          return BigInt(3);
        },
        /*pollFunc:*/ (rustFuture, cb, handle: UniffiHandle) => {
          calls.push(`poll ${rustFuture}`);
          setTimeout(() => cb(handle, 0 /* READY */), 0);
        },
        /*cancelFunc:*/ (rustFuture) => calls.push(`cancel ${rustFuture}`),
        /*completeFunc:*/ async (rustFuture, status) => {
          calls.push(`complete ${rustFuture}`);
          status.code = 0;
          return 1;
        },
        /*freeFunc:*/ (rustFuture) => calls.push(`free ${rustFuture}`),
        /*liftFunc:*/ (n: number) => n,
        liftString,
        { signal: abortController.signal },
      );
      // rustFutureFunc is still awaiting its own promise here, so the abort
      // listener must already be armed or this cancellation is silently lost.
      abortController.abort();
      await promise;
      t.assertTrue(
        calls.includes("cancel 3"),
        () => `expected a cancel call, got: ${calls.join(",")}`,
      );
      t.end();
    },
  );
})();
