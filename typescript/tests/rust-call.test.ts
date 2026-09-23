// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
import { asyncTest, test } from "../testing/asserts";
import { UniffiRustCaller, type UniffiRustCallStatus } from "../src/rust-call";

test("synchronous calls keep synchronous return values", (t) => {
  const caller = new UniffiRustCaller(() => ({ code: 0 }));
  t.assertEqual(
    caller.rustCall(() => 42),
    42,
  );
});

(async () => {
  await asyncTest(
    "async status is read and consumed only after settlement",
    async (t) => {
      let consumed = 0;
      let release!: () => void;
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      const caller = new UniffiRustCaller<UniffiRustCallStatus>(
        () => ({ code: 0 }),
        (status) => {
          consumed++;
          return status;
        },
      );
      const expected = new Error("lifted error");
      const promise = caller.rustCallAsyncWithError(
        () => expected,
        async (status) => {
          await gate;
          status.code = 1;
          status.errorBuf = new Uint8Array([1]);
        },
      );
      t.assertEqual(consumed, 0);
      release();
      let caught: unknown;
      try {
        await promise;
      } catch (error) {
        caught = error;
      }
      t.assertEqual(caught, expected);
      t.assertEqual(consumed, 1);
      t.end();
    },
  );

  await asyncTest(
    "async cleanup runs once on rejection and synchronous throw",
    async (t) => {
      let consumed = 0;
      const caller = new UniffiRustCaller(
        () => ({ code: 0 }),
        (status) => {
          consumed++;
          return status;
        },
      );
      const expected = new Error("import rejection");
      for (const call of [
        () => Promise.reject(expected),
        () => {
          throw expected;
        },
      ]) {
        let caught: unknown;
        try {
          await caller.rustCallAsync(call);
        } catch (error) {
          caught = error;
        }
        t.assertEqual(caught, expected);
      }
      t.assertEqual(consumed, 2);
      t.end();
    },
  );

  await asyncTest(
    "overlapping calls have independent status and cleanup",
    async (t) => {
      const consumed: UniffiRustCallStatus[] = [];
      const caller = new UniffiRustCaller<UniffiRustCallStatus>(
        () => ({ code: 0 }),
        (status) => {
          consumed.push(status);
          return status;
        },
      );
      let finish!: (value: number) => void;
      const first = caller.rustCallAsync(
        () =>
          new Promise<number>((resolve) => {
            finish = resolve;
          }),
      );
      t.assertEqual(await caller.rustCallAsync(async () => 2), 2);
      t.assertEqual(consumed.length, 1);
      finish(1);
      t.assertEqual(await first, 1);
      t.assertEqual(consumed.length, 2);
      t.assertTrue(consumed[0] !== consumed[1]);
      t.end();
    },
  );
})();
