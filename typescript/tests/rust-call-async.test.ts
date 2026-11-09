/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "../testing/polyfills";
import { asyncTest } from "../testing/asserts";
import {
  CALL_ERROR,
  UniffiRustCaller,
  type UniffiRustCallStatus,
} from "../src/index";

const caller = new UniffiRustCaller<UniffiRustCallStatus>(() => ({ code: 0 }));
const liftString = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

(async () => {
  await asyncTest(
    "rustCallAsync resolves a promise-returning caller",
    async (t) => {
      const v = await caller.rustCallAsync(async (status) => {
        status.code = 0;
        return 42;
      }, liftString);
      t.assertEqual(v, 42);
      t.end();
    },
  );

  await asyncTest("rustCallAsync resolves a plain caller too", async (t) => {
    const v = await caller.rustCallAsync(() => "sync", liftString);
    t.assertEqual(v, "sync");
    t.end();
  });

  await asyncTest(
    "rustCallAsync checks the status after the caller resolves",
    async (t) => {
      // Hermes doesn't build the prototype chain for classes returned from the
      // UniffiInternalError IIFE, so match by message rather than instanceof
      // (see typescript/tests/handlemap.test.ts for the same pattern).
      await t.assertThrowsAsync(
        (e) => e instanceof Error && e.message === "Rust panic",
        () =>
          caller.rustCallAsync(async (status) => {
            await Promise.resolve();
            status.code = 2; // CALL_UNEXPECTED_ERROR
            return 0;
          }, liftString),
      );
      t.end();
    },
  );

  await asyncTest(
    "rustCallWithErrorAsync lifts the error buffer",
    async (t) => {
      // Same Hermes prototype-chain limitation as above: match by message.
      class MyErr extends Error {
        constructor() {
          super("myerr");
        }
      }
      await t.assertThrowsAsync(
        (e) => e instanceof Error && e.message === "myerr",
        () =>
          caller.rustCallWithErrorAsync(
            () => new MyErr(),
            async (status) => {
              status.code = CALL_ERROR;
              status.errorBuf = new Uint8Array([1]);
              return 0;
            },
            liftString,
          ),
      );
      t.end();
    },
  );

  await asyncTest(
    "rustCallAsync rejects when the caller rejects",
    async (t) => {
      await t.assertThrowsAsync(
        (e) => e instanceof Error && e.message === "boom",
        () =>
          caller.rustCallAsync(
            () => Promise.reject(new Error("boom")),
            liftString,
          ),
      );
      t.end();
    },
  );
})();
