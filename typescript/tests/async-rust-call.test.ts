// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
import { asyncTest } from "../testing/asserts";
import {
  uniffiRustCallAsync,
  uniffiRustFutureHandleCount,
} from "../src/async-rust-call";
import { UniffiRustCaller, type UniffiRustCallStatus } from "../src/rust-call";
import { UniffiInternalError } from "../src/errors";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

(async () => {
  await asyncTest("poll waits for both signals in either order", async (t) => {
    for (const callbackFirst of [true, false]) {
      const gate = deferred<void>();
      let notify!: () => void;
      let complete = 0,
        freed = 0,
        consumed = 0;
      const caller = new UniffiRustCaller(
        () => ({ code: 0 }),
        (s) => {
          consumed++;
          return s;
        },
      );
      const promise = uniffiRustCallAsync(
        caller,
        () => 1n,
        (_future, cb, handle) => {
          notify = () => cb(handle, 0);
          return gate.promise;
        },
        () => {},
        () => {
          complete++;
          return 42;
        },
        () => {
          freed++;
        },
        (v) => v,
        () => "",
        undefined,
        undefined,
        true,
      );
      if (callbackFirst) notify();
      else gate.resolve();
      await tick();
      t.assertEqual(complete, 0);
      t.assertEqual(freed, 0);
      if (callbackFirst) gate.resolve();
      else notify();
      t.assertEqual(await promise, 42);
      t.assertEqual(complete, 1);
      t.assertEqual(freed, 1);
      t.assertEqual(consumed, 1);
      t.assertEqual(uniffiRustFutureHandleCount(), 0);
    }
    t.end();
  });

  await asyncTest(
    "fatal poll failures remove handles and never reenter Rust",
    async (t) => {
      for (const synchronous of [true, false]) {
        for (const notifyFirst of [true, false]) {
          const cause = new Error("trap");
          let late!: () => void;
          let freed = 0,
            completed = 0,
            cancelled = 0;
          const abort = new AbortController();
          const promise = uniffiRustCallAsync(
            new UniffiRustCaller(() => ({ code: 0 })),
            () => 1n,
            (_future, cb, handle) => {
              late = () => cb(handle, 0);
              if (notifyFirst) late();
              if (synchronous) throw cause;
              return Promise.reject(cause);
            },
            () => {
              cancelled++;
            },
            () => {
              completed++;
            },
            () => {
              freed++;
            },
            (v) => v,
            () => "",
            { signal: abort.signal },
            undefined,
            true,
          );
          let caught: unknown;
          try {
            await promise;
          } catch (e) {
            caught = e;
          }
          t.assertTrue(caught instanceof UniffiInternalError.JspiPollError);
          t.assertEqual((caught as Error & { cause: unknown }).cause, cause);
          abort.abort();
          late();
          t.assertEqual(freed + completed + cancelled, 0);
          t.assertEqual(uniffiRustFutureHandleCount(), 0);
        }
      }
      t.end();
    },
  );

  await asyncTest(
    "abort during poll waits then consumes cancelled status",
    async (t) => {
      const gate = deferred<void>();
      const abort = new AbortController();
      let notify!: () => void;
      let freed = 0,
        completed = 0,
        consumed = 0;
      const promise = uniffiRustCallAsync(
        new UniffiRustCaller<UniffiRustCallStatus>(
          () => ({ code: 0 }),
          (s) => {
            consumed++;
            return s;
          },
        ),
        () => 1n,
        (_future, cb, handle) => {
          notify = () => cb(handle, 0);
          return gate.promise;
        },
        () => notify(),
        (_future, status) => {
          completed++;
          status.code = 3;
        },
        () => {
          freed++;
        },
        (v) => v,
        () => "",
        { signal: abort.signal },
        undefined,
        true,
      );
      const outcome = promise.catch((e) => e);
      abort.abort();
      await tick();
      t.assertEqual(freed + completed, 0);
      gate.resolve();
      t.assertTrue((await outcome) instanceof UniffiInternalError.AbortError);
      t.assertEqual(freed, 1);
      t.assertEqual(consumed, 1);
      t.assertEqual(uniffiRustFutureHandleCount(), 0);
      t.end();
    },
  );

  await asyncTest(
    "sync polling repolls after wake and frees if lifting fails",
    async (t) => {
      let polls = 0,
        freed = 0;
      const failure = new Error("lift");
      let caught;
      try {
        await uniffiRustCallAsync(
          new UniffiRustCaller(() => ({ code: 0 })),
          () => 1n,
          (_future, cb, handle) => cb(handle, polls++ === 0 ? 1 : 0),
          () => {},
          () => 42,
          () => {
            freed++;
          },
          () => {
            throw failure;
          },
          () => "",
        );
      } catch (e) {
        caught = e;
      }
      t.assertEqual(caught, failure);
      t.assertEqual(polls, 2);
      t.assertEqual(freed, 1);
      t.assertEqual(uniffiRustFutureHandleCount(), 0);
      t.end();
    },
  );
})();
