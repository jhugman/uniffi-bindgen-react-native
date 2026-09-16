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
        /*cancelFunc:*/ () => {
          calls.push("cancel");
        },
        /*completeFunc:*/ async (rustFuture, status) => {
          calls.push(`complete ${rustFuture}`);
          status.code = 0;
          return 99;
        },
        /*freeFunc:*/ (rustFuture) => {
          calls.push(`free ${rustFuture}`);
        },
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
        /*cancelFunc:*/ (rustFuture) => {
          calls.push(`cancel ${rustFuture}`);
        },
        /*completeFunc:*/ async (rustFuture, status) => {
          calls.push(`complete ${rustFuture}`);
          // A cancelled Rust future completes with CALL_CANCELLED.
          status.code = 3;
          return 1;
        },
        /*freeFunc:*/ (rustFuture) => {
          calls.push(`free ${rustFuture}`);
        },
        /*liftFunc:*/ (n: number) => n,
        liftString,
        { signal: abortController.signal },
      );
      // rustFutureFunc is still awaiting its own promise here, so the abort
      // arrives before the listener exists; the `signal.aborted` check catches it.
      abortController.abort();
      // Hermes does not give errors the right prototype chain, so match by name.
      await t.assertThrowsAsync(
        (e) => e.name === "AbortError",
        () => promise,
      );
      t.assertTrue(
        calls.includes("cancel 3"),
        () => `expected a cancel call, got: ${calls.join(",")}`,
      );
      t.end();
    },
  );

  await asyncTest(
    "a void call rejecting over a closed port does not sink the result",
    async (t) => {
      const logged: string[] = [];
      const consoleError = console.error;
      console.error = (...args: any[]) => {
        logged.push(args.map((a) => String(a)).join(" "));
      };
      try {
        const result = await uniffiRustCallAsync(
          caller,
          /*rustFutureFunc:*/ async () => BigInt(5),
          /*pollFunc:*/ (rustFuture, cb, handle: UniffiHandle) => {
            setTimeout(() => cb(handle, 0 /* READY */), 0);
          },
          /*cancelFunc:*/ () => {},
          /*completeFunc:*/ async (rustFuture, status) => {
            status.code = 0;
            return 42;
          },
          // A closed port rejects the free call; nothing else is listening.
          /*freeFunc:*/ () => Promise.reject(new Error("port closed")),
          /*liftFunc:*/ (n: number) => n,
          liftString,
        );
        t.assertEqual(result, 42);
        // Let the rejection handler attached by the call run.
        await new Promise<void>((resolve) => setTimeout(() => resolve(), 0));
      } finally {
        console.error = consoleError;
      }
      t.assertEqual(logged.length, 1, () => `logged: ${logged.join(" | ")}`);
      t.assertTrue(
        logged[0].indexOf("port closed") >= 0,
        () => `expected the rejection to be reported, got: ${logged[0]}`,
      );
      t.end();
    },
  );
})();
