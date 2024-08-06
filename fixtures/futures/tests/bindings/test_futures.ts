/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import myModule, {
  alwaysReady,
  asStringUsingTrait,
  asyncNewMegaphone,
  AsyncParser,
  cancelDelayUsingTrait,
  delayUsingTrait,
  fallibleMe,
  FallibleMegaphone,
  fallibleStruct,
  getSayAfterTraits,
  getSayAfterUdlTraits,
  greet,
  Megaphone,
  MyError,
  newMegaphone,
  newMyRecord,
  ParserError,
  sayAfter,
  sayAfterWithTokio,
  SharedResourceOptions,
  sleep,
  tryDelayUsingTrait,
  tryFromStringUsingTrait,
  useSharedResource,
  void_,
} from "../../generated/futures";
import { asyncTest, xasyncTest, Asserts, test } from "@/asserts";
import {
  uniffiRustFutureHandleCount,
  uniffiForeignFutureHandleCount,
} from "uniffi-bindgen-react-native";
import { console } from "@/hermes";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

function checkRemainingFutures(t: Asserts) {
  t.assertEqual(
    0,
    uniffiRustFutureHandleCount(),
    "Number of remaining futures should be zero",
  );
  t.assertEqual(
    0,
    uniffiForeignFutureHandleCount(),
    "Number of remaining foreign futures should be zero",
  );
}

(async () => {
  await asyncTest("alwaysReady", async (t) => {
    const result = await alwaysReady();
    t.assertTrue(result);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("newMyRecord", async (t) => {
    const record = await newMyRecord("my string", 42);
    t.assertEqual(record.a, "my string");
    t.assertEqual(record.b, 42);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("void", async (t) => {
    await t.asyncMeasure(void_, 0, 500);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("sleep", async (t) => {
    await t.asyncMeasure(async () => sleep(500), 500, 50);
    checkRemainingFutures(t);
    t.end();
  });

  test("sync greet", (t) => {
    t.measure(() => greet("Hello"), 0, 10);
  });

  await asyncTest("Sequential futures", async (t) => {
    await t.asyncMeasure(
      async () => {
        t.assertEqual("Hello, Alice!", await sayAfter(500, "Alice"));
        t.assertEqual("Hello, Bob!", await sayAfter(500, "Bob"));
      },
      1000,
      50,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Concurrent futures", async (t) => {
    await t.asyncMeasure(
      async () => {
        const alice = sayAfter(400, "Alice");
        const bob = sayAfter(600, "Bob");
        const [helloAlice, helloBob] = await Promise.all([alice, bob]);
        t.assertEqual("Hello, Alice!", helloAlice);
        t.assertEqual("Hello, Bob!", helloBob);
      },
      600,
      50,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Async methods", async (t) => {
    const megaphone = newMegaphone();
    let helloAlice = await t.asyncMeasure(
      async () => megaphone.sayAfter(500, "Alice"),
      500,
      20,
    );
    t.assertEqual("HELLO, ALICE!", helloAlice);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Async trait interface methods", async (t) => {
    const traits = getSayAfterTraits();

    await t.asyncMeasure(
      async () => {
        let result1 = await traits[0].sayAfter(300, "Alice");
        let result2 = await traits[1].sayAfter(200, "Bob");

        t.assertEqual(result1, "Hello, Alice!");
        t.assertEqual(result2, "Hello, Bob!");
      },
      500,
      50,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("UDL-defined async trait interface methods", async (t) => {
    const traits = getSayAfterUdlTraits();

    await t.asyncMeasure(
      async () => {
        let result1 = await traits[0].sayAfter(300, "Alice");
        let result2 = await traits[1].sayAfter(200, "Bob");

        t.assertEqual(result1, "Hello, Alice!");
        t.assertEqual(result2, "Hello, Bob!");
      },
      500,
      50,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Object with a fallible async ctor.", async (t) => {
    try {
      await FallibleMegaphone.create();
      t.fail("Expected an error");
    } catch (e: any) {
      // OK
    }
    checkRemainingFutures(t);
    t.end();
  });

  // AsyncParser.
  class TsAsyncParser implements AsyncParser {
    constructor(public completedDelays: number = 0) {}
    async asString(delayMs: number, value: number): Promise<string> {
      await this.doDelay(delayMs);
      return value.toString();
    }
    async tryFromString(delayMs: number, value: string): Promise<number> {
      if (value == "force-panic") {
        throw new Error("force-panic");
      }
      if (value == "force-unexpected-exception") {
        throw new ParserError.UnexpectedError();
      }
      const v = this.parseInt(value);
      await this.doDelay(delayMs);
      return v;
    }
    async delay(delayMs: number): Promise<void> {
      await this.doDelay(delayMs);
    }
    async tryDelay(delayMs: string): Promise<void> {
      await this.doDelay(this.parseInt(delayMs));
    }

    toString(): string {
      return "TsAsyncParser";
    }

    private async doDelay(ms: number): Promise<void> {
      await delayPromise(ms);
      this.completedDelays += 1;
    }

    private parseInt(value: string): number {
      const num = Number.parseInt(value);
      if (Number.isNaN(num)) {
        throw new ParserError.NotAnInt();
      }
      return num;
    }
  }

  await asyncTest("Async callbacks", async (t) => {
    const traitObj = new TsAsyncParser();

    const result = await asStringUsingTrait(traitObj, 1, 42);
    t.assertEqual(result, "42");

    const result2 = await tryFromStringUsingTrait(traitObj, 1, "42");
    t.assertEqual(result2, 42);

    await delayUsingTrait(traitObj, 1);
    await tryDelayUsingTrait(traitObj, "1");
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Async callbacks with errors", async (t) => {
    const traitObj = new TsAsyncParser();

    try {
      await tryFromStringUsingTrait(traitObj, 1, "force-panic");
      t.fail("No error detected");
    } catch (e: any) {
      // OK
      t.assertTrue(ParserError.UnexpectedError.instanceOf(e));
    }

    try {
      await tryFromStringUsingTrait(traitObj, 1, "fourty-two");
      t.fail("No error detected");
    } catch (e: any) {
      t.assertTrue(ParserError.NotAnInt.instanceOf(e));
    }

    await t.assertThrowsAsync(
      ParserError.NotAnInt.instanceOf,
      async () => await tryFromStringUsingTrait(traitObj, 1, "fourty-two"),
    );
    await t.assertThrowsAsync(
      ParserError.UnexpectedError.instanceOf,
      async () =>
        await tryFromStringUsingTrait(
          traitObj,
          1,
          "force-unexpected-exception",
        ),
    );

    try {
      await tryDelayUsingTrait(traitObj, "one");
      t.fail("Expected previous statement to throw");
    } catch (e: any) {
      // Expected
    }
    checkRemainingFutures(t);
    t.end();
  });

  /**
   * Skipping this test as it is testing an abort being propogated to Javascript.
   *
   * Cancellable promises aren't a standard in JS, so there is nothing to cancel.
   *
   * Even then, the single threaded-ness of JS means that this test would rely on client
   * code, i.e. `doDelay` checking if the Promise had been cancelled before incrementing
   * the `completedDelays` count.
   */
  await xasyncTest("cancellation of async JS callbacks", async (t) => {
    const traitObj = new TsAsyncParser();

    // #JS_TASK_CANCELLATION
    const completedDelaysBefore = traitObj.completedDelays;
    const promise = cancelDelayUsingTrait(traitObj, 100);
    // sleep long enough so that the `delay()` call would finish if it wasn't cancelled.
    await delayPromise(1000);
    await promise;
    // If the task was cancelled, then completedDelays won't have increased
    // however, this is cancelling the async callback, which doesn't really make any sense
    // in Javascript.
    t.assertEqual(
      traitObj.completedDelays,
      completedDelaysBefore,
      "Delay should have been cancelled",
    );

    // Test that all handles here cleaned up
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("async function returning an object", async (t) => {
    const megaphone = await asyncNewMegaphone();
    const result = await megaphone.fallibleMe(false);
    t.assertEqual(result, 42);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest(
    "async function returning an object with primary async connstructor",
    async (t) => {
      const megaphone = await Megaphone.create();
      const result = await megaphone.fallibleMe(false);
      t.assertEqual(result, 42);
      checkRemainingFutures(t);
      t.end();
    },
  );

  await asyncTest(
    "async function returning an object with secondary async connstructor",
    async (t) => {
      const megaphone = await Megaphone.secondary();

      const result = await megaphone.fallibleMe(false);
      t.assertEqual(result, 42);
      checkRemainingFutures(t);
      t.end();
    },
  );

  await asyncTest("With the Tokio runtime", async (t) => {
    const helloAlice = await t.asyncMeasure(
      async () => sayAfterWithTokio(500, "Alice"),
      500,
      20,
    );
    t.assertEqual("Hello, Alice (with Tokio)!", helloAlice);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("fallible function… which doesn't throw", async (t) => {
    const result = await t.asyncMeasure(async () => fallibleMe(false), 0, 100);
    t.assertEqual(42, result);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("fallible method… which doesn't throw", async (t) => {
    const m = await fallibleStruct(false);
    const result = await m.fallibleMe(false);
    t.assertEqual(42, result);
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest(
    "fallible method… which doesn't throw, part II",
    async (t) => {
      const megaphone = newMegaphone();
      const result = await t.asyncMeasure(
        async () => megaphone.fallibleMe(false),
        0,
        100,
      );
      t.assertEqual(42, result);
      checkRemainingFutures(t);
      t.end();
    },
  );

  await asyncTest("fallible function… which does throw", async (t) => {
    await t.asyncMeasure(
      async () =>
        await t.assertThrowsAsync(MyError.Foo.instanceOf, async () =>
          fallibleMe(true),
        ),
      0,
      100,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("fallible method… which does throw", async (t) => {
    await t.assertThrowsAsync(
      MyError.Foo.instanceOf,
      async () => await fallibleStruct(true),
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("fallible method… which does throw, part II", async (t) => {
    const megaphone = newMegaphone();
    await t.asyncMeasure(
      async () =>
        await t.assertThrowsAsync(MyError.Foo.instanceOf, async () =>
          megaphone.fallibleMe(true),
        ),
      0,
      100,
    );
    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest(
    "a future that uses a lock and that is not cancelled",
    async (t) => {
      const task1 = useSharedResource(
        SharedResourceOptions.create({
          releaseAfterMs: 100,
          timeoutMs: 1000,
        }),
      );
      const task2 = useSharedResource(
        SharedResourceOptions.create({ releaseAfterMs: 0, timeoutMs: 1000 }),
      );
      await Promise.all([task1, task2]);

      checkRemainingFutures(t);
      t.end();
    },
  );

  await xasyncTest(
    "a future that uses a lock and that is cancelled from JS",
    async (t) => {
      const task1 = useSharedResource(
        SharedResourceOptions.create({
          releaseAfterMs: 5000,
          timeoutMs: 100,
        }),
      );
      // #RUST_TASK_CANCELLATION
      //
      // Again this test is not really applicable for JS, as it has no standard way of
      // cancelling a task.
      // task1.cancel()

      // Try accessing the shared resource again.  The initial task should release the shared resource
      // before the timeout expires.
      const task2 = useSharedResource(
        SharedResourceOptions.create({ releaseAfterMs: 0, timeoutMs: 1000 }),
      );

      await Promise.allSettled([task1, task2]);
      checkRemainingFutures(t);
      t.end();
    },
  );

  await xasyncTest(
    "a future that uses a lock and that is cancelled by Rust",
    async (t) => {
      // #RUST_TASK_CANCELLATION
      //
      // We should be able to see this test pass if Rust is calling a
      // timeout a cancellation.
      //
      await t.assertThrowsAsync(
        (err) => {
          t.assertEqual(err.message, "cancelled");
          return true;
        },
        async () => {
          await useSharedResource(
            SharedResourceOptions.create({
              releaseAfterMs: 1000,
              timeoutMs: 100,
            }),
          );
        },
      );
      checkRemainingFutures(t);
      t.end();
    },
  );
})();
