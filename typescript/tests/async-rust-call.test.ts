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
  uniffiDriveRustFuture,
  uniffiRustCallAsyncEntry,
  uniffiRustFutureHandleCount,
  type UniffiRustFutureFns,
  type UniffiRustCallStatus,
  type UniffiHandle,
} from "../src/index";

const caller = new UniffiRustCaller<UniffiRustCallStatus>(() => ({ code: 0 }));
const liftString = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

// Fake future functions that record each call as "<verb> <future>".
function fns(
  calls: string[],
  opts: {
    poll?: UniffiRustFutureFns<number, UniffiRustCallStatus>["poll"];
    cancel?: UniffiRustFutureFns<number, UniffiRustCallStatus>["cancel"];
    complete?: (status: UniffiRustCallStatus) => number;
  } = {},
): UniffiRustFutureFns<number, UniffiRustCallStatus> {
  return {
    poll: (rustFuture, cb, handle) => {
      calls.push(`poll ${rustFuture}`);
      if (opts.poll) return opts.poll(rustFuture, cb, handle);
      setTimeout(() => cb(handle, 0 /* READY */), 0);
    },
    cancel: (rustFuture) => {
      calls.push(`cancel ${rustFuture}`);
      if (opts.cancel) return opts.cancel(rustFuture);
    },
    complete: (rustFuture, status) => {
      calls.push(`complete ${rustFuture}`);
      if (opts.complete) return opts.complete(status);
      status.code = 0;
      return 5;
    },
    free: (rustFuture) => {
      calls.push(`free ${rustFuture}`);
    },
  };
}

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

  await asyncTest("a poll that rejects rejects the call", async (t) => {
    const calls: string[] = [];
    const consoleError = console.error;
    console.error = () => {};
    const handlesBefore = uniffiRustFutureHandleCount();
    let outcome: string;
    try {
      const promise = uniffiRustCallAsync(
        caller,
        /*rustFutureFunc:*/ async () => BigInt(9),
        // A player over a port answers a poll that trapped with a rejection,
        // and never fires the continuation.
        /*pollFunc:*/ () => Promise.reject(new Error("unreachable")),
        /*cancelFunc:*/ () => {},
        /*completeFunc:*/ async (rustFuture, status) => {
          calls.push(`complete ${rustFuture}`);
          status.code = 0;
          return 1;
        },
        /*freeFunc:*/ (rustFuture) => {
          calls.push(`free ${rustFuture}`);
        },
        /*liftFunc:*/ (n: number) => n,
        liftString,
      );
      outcome = await Promise.race([
        promise.then(
          () => "resolved",
          (e: any) => `rejected: ${e.message}`,
        ),
        new Promise<string>((resolve) =>
          setTimeout(() => resolve("still pending"), 100),
        ),
      ]);
    } finally {
      console.error = consoleError;
    }
    t.assertEqual(outcome, "rejected: unreachable");
    t.assertEqual(calls.join(","), "free 9");
    t.assertEqual(uniffiRustFutureHandleCount(), handlesBefore);
    t.end();
  });

  await asyncTest(
    "the driver resolves a future ready on its first poll and frees it",
    async (t) => {
      const calls: string[] = [];
      const status = { code: 0 };
      const before = uniffiRustFutureHandleCount();
      t.assertEqual(
        await uniffiDriveRustFuture(BigInt(7), fns(calls), status),
        5,
      );
      t.assertEqual(calls.join(","), "poll 7,complete 7,free 7");
      t.assertEqual(uniffiRustFutureHandleCount(), before);
      t.end();
    },
  );

  await asyncTest(
    "MAYBE_READY is re-polled on a microtask, with no macrotask in between",
    async (t) => {
      const calls: string[] = [];
      let polls = 0;
      const f = fns(calls, {
        poll: (_rustFuture, cb, handle) => {
          polls += 1;
          const result = polls < 3 ? 1 /* MAYBE_READY */ : 0; /* READY */
          Promise.resolve().then(() => cb(handle, result));
        },
      });
      let macrotaskRan = false;
      setTimeout(() => {
        macrotaskRan = true;
      }, 0);
      await uniffiDriveRustFuture(BigInt(7), f, { code: 0 });
      t.assertEqual(polls, 3);
      t.assertFalse(macrotaskRan);
      t.end();
    },
  );

  await asyncTest(
    "a poll that rejects rejects the driver and still frees",
    async (t) => {
      const calls: string[] = [];
      const f = fns(calls, {
        poll: () => Promise.reject(new Error("unreachable")),
      });
      await t.assertThrowsAsync(
        (e) => e.message === "unreachable",
        () => uniffiDriveRustFuture(BigInt(9), f, { code: 0 }),
      );
      t.assertEqual(calls.join(","), "poll 9,free 9");
      t.end();
    },
  );

  await asyncTest(
    "an abort mid-flight cancels; the cancelled status comes back through complete",
    async (t) => {
      const calls: string[] = [];
      let parked:
        | { cb: (h: UniffiHandle, r: number) => void; handle: UniffiHandle }
        | undefined;
      const f = fns(calls, {
        poll: (_rustFuture, cb, handle) => {
          parked = { cb, handle };
        },
        cancel: () => {
          setTimeout(() => parked!.cb(parked!.handle, 0), 0);
        },
        complete: (status) => {
          status.code = 3;
          return 1;
        },
      });
      const controller = new AbortController();
      const status = { code: 0 };
      const p = uniffiDriveRustFuture(BigInt(7), f, status, controller.signal);
      setTimeout(() => controller.abort(), 5);
      // The driver does not throw on a non-zero status; the caller reads it.
      t.assertEqual(await p, 1);
      t.assertEqual(status.code, 3);
      t.assertTrue(calls.includes("cancel 7"));
      t.assertEqual(calls[calls.length - 1], "free 7");
      t.end();
    },
  );

  await asyncTest("an abort after completion is ignored", async (t) => {
    const calls: string[] = [];
    const controller = new AbortController();
    await uniffiDriveRustFuture(
      BigInt(7),
      fns(calls),
      { code: 0 },
      controller.signal,
    );
    controller.abort();
    t.assertFalse(calls.includes("cancel 7"));
    t.end();
  });

  await asyncTest(
    "a signal already aborted when the driver starts cancels before the first poll",
    async (t) => {
      const calls: string[] = [];
      const controller = new AbortController();
      controller.abort();
      await uniffiDriveRustFuture(
        BigInt(7),
        fns(calls),
        { code: 0 },
        controller.signal,
      );
      t.assertEqual(calls[0], "cancel 7");
      t.end();
    },
  );

  await asyncTest(
    "a throwing complete rejects the driver and still frees",
    async (t) => {
      const calls: string[] = [];
      const f = fns(calls, {
        complete: () => {
          throw new Error("boom");
        },
      });
      await t.assertThrowsAsync(
        (e) => e.message === "boom",
        () => uniffiDriveRustFuture(BigInt(7), f, { code: 0 }),
      );
      t.assertEqual(calls[calls.length - 1], "free 7");
      t.end();
    },
  );

  await asyncTest("the entry helper lifts the completed value", async (t) => {
    const r = await uniffiRustCallAsyncEntry(
      caller,
      async (status, signal) => {
        status.code = 0;
        t.assertEqual(signal, undefined);
        return 41;
      },
      (n: number) => n + 1,
      liftString,
    );
    t.assertEqual(r, 42);
    t.end();
  });
  await asyncTest(
    "the entry helper rejects an aborted signal without calling the entry",
    async (t) => {
      const c = new AbortController();
      c.abort();
      let called = false;
      await t.assertThrowsAsync(
        (e) => e.name === "AbortError",
        () =>
          uniffiRustCallAsyncEntry(
            caller,
            async () => {
              called = true;
              return 1;
            },
            (n: number) => n,
            liftString,
            { signal: c.signal },
          ),
      );
      t.assertFalse(called);
      t.end();
    },
  );
  await asyncTest(
    "the entry helper passes the signal through and turns CALL_CANCELLED into AbortError",
    async (t) => {
      const c = new AbortController();
      await t.assertThrowsAsync(
        (e) => e.name === "AbortError",
        () =>
          uniffiRustCallAsyncEntry(
            caller,
            async (status, signal) => {
              t.assertEqual(signal, c.signal);
              status.code = 3;
              return 0;
            },
            (n: number) => n,
            liftString,
            { signal: c.signal },
          ),
      );
      t.end();
    },
  );
  await asyncTest(
    "the entry helper rejects a player that returns the future handle instead of a promise",
    async (t) => {
      let lifted = false;
      await t.assertThrowsAsync(
        (e) => /@ubjs\/wasm is too old/.test(e.message),
        () =>
          uniffiRustCallAsyncEntry(
            caller,
            // What a player that predates async entries returns.
            () => BigInt(7),
            (n: bigint) => {
              lifted = true;
              return n;
            },
            liftString,
          ),
      );
      t.assertFalse(lifted);
      t.end();
    },
  );
  await asyncTest(
    "the entry helper hands CALL_ERROR's buffer to the error handler",
    async (t) => {
      await t.assertThrowsAsync(
        (e) => e.message === "lifted:7",
        () =>
          uniffiRustCallAsyncEntry(
            caller,
            async (status) => {
              status.code = 1;
              status.errorBuf = new Uint8Array([7]);
              return 0;
            },
            (n: number) => n,
            liftString,
            undefined,
            (buf) => new Error(`lifted:${buf[0]}`),
          ),
      );
      t.end();
    },
  );
})();
