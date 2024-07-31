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
import { asyncTest, xasyncTest } from "@/asserts";
import { console } from "@/hermes";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

asyncTest("alwaysReady", async (t) => {
  const result = await alwaysReady();
  t.assertTrue(result);
  t.end();
});

asyncTest("newMyRecord", async (t) => {
  const record = await newMyRecord("my string", 42);
  t.assertEqual(record.a, "my string");
  t.assertEqual(record.b, 42);
  t.end();
});

asyncTest("void", async (t) => {
  await t.asyncMeasure(void_, 0, 500);
  t.end();
});

asyncTest("sleep", async (t) => {
  await t.asyncMeasure(() => sleep(2000), 2000, 50);
  t.end();
});

asyncTest("sync greet", async (t) => {
  await t.measure(() => greet("Hello"), 0, 10);
  t.end();
});

asyncTest("Sequential futures", async (t) => {
  t.asyncMeasure(
    async () => {
      t.assertEqual("Hello, Alice!", await sayAfter(1000, "Alice"));
      t.assertEqual("Hello, Bob!", await sayAfter(2000, "Bob"));
    },
    3000,
    20,
  );
  t.end();
});

asyncTest("Concurrent futures", async (t) => {
  t.asyncMeasure(
    async () => {
      const alice = sayAfter(1000, "Alice");
      const bob = sayAfter(2000, "Bob");
      const [helloAlice, helloBob] = await Promise.all([alice, bob]);
      t.assertEqual("Hello, Alice!", helloAlice);
      t.assertEqual("Hello, Bob!", helloBob);
    },
    2000,
    20,
  );
  t.end();
});

asyncTest("Async methods", async (t) => {
  const megaphone = newMegaphone();
  let helloAlice = await t.asyncMeasure(
    async () => megaphone.sayAfter(2000, "Alice"),
    2000,
    20,
  );
  t.assertEqual("HELLO, ALICE!", helloAlice);
  t.end();
});

asyncTest("Async trait interface methods", async (t) => {
  const traits = getSayAfterTraits();

  t.asyncMeasure(
    async () => {
      let result1 = await traits[0].sayAfter(1000, "Alice");
      let result2 = await traits[1].sayAfter(1000, "Bob");

      t.assertEqual(result1, "Hello, Alice!");
      t.assertEqual(result2, "Hello, Bob!");
    },
    2000,
    20,
  );
  t.end();
});

asyncTest("UDL-defined async trait interface methods", async (t) => {
  const traits = getSayAfterUdlTraits();

  t.asyncMeasure(
    async () => {
      let result1 = await traits[0].sayAfter(1000, "Alice");
      let result2 = await traits[1].sayAfter(1000, "Bob");

      t.assertEqual(result1, "Hello, Alice!");
      t.assertEqual(result2, "Hello, Bob!");
    },
    2000,
    20,
  );
  t.end();
});

asyncTest("Object with a fallible async ctor.", async (t) => {
  try {
    await FallibleMegaphone.create();
    t.fail("Expected an error");
  } catch (e: any) {
    // OK
  }
  t.end();
});

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

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

asyncTest("Async callbacks", async (t) => {
  const traitObj = new TsAsyncParser();

  const result = await asStringUsingTrait(traitObj, 1, 42);
  t.assertEqual(result, "42");

  const result2 = await tryFromStringUsingTrait(traitObj, 1, "42");
  t.assertEqual(result2, 42);

  await delayUsingTrait(traitObj, 1);
  await tryDelayUsingTrait(traitObj, "1");

  t.end();
});

asyncTest("Async callbacks with errors", async (t) => {
  const traitObj = new TsAsyncParser();

  try {
    await tryFromStringUsingTrait(traitObj, 1, "force-panic");
    t.fail("No error detected");
  } catch (e: any) {
    // OK
  }

  try {
    await tryFromStringUsingTrait(traitObj, 1, "fourty-two");
    t.fail("No error detected");
  } catch (e: any) {
    // OK
  }

  await t.assertThrowsAsync(
    ParserError.NotAnInt.instanceOf,
    async () => await tryFromStringUsingTrait(traitObj, 1, "fourty-two"),
  );
  await t.assertThrowsAsync(
    ParserError.UnexpectedError.instanceOf,
    async () =>
      await tryFromStringUsingTrait(traitObj, 1, "force-unexpected-exception"),
  );

  try {
    await tryDelayUsingTrait(traitObj, "one");
    t.fail("Expected previous statement to throw");
  } catch (/*ParserError.NotAnInt*/ e: any) {
    // Expected
  }

  t.end();
});

xasyncTest("Async callbacks cancellation", async (t) => {
  const traitObj = new TsAsyncParser();

  const completedDelaysBefore = traitObj.completedDelays;
  await cancelDelayUsingTrait(traitObj, 100);
  // sleep long enough so that the `delay()` call would finish if it wasn't cancelled.
  await delayPromise(1000);
  // If the task was cancelled, then completedDelays won't have increased
  t.assertEqual(traitObj.completedDelays, completedDelaysBefore);

  // Test that all handles here cleaned up
  // assert(uniffiForeignFutureHandleCountFutures() == 0)

  t.end();
});

asyncTest("async function returning an object", async (t) => {
  const megaphone = await asyncNewMegaphone();
  const result = await megaphone.fallibleMe(false);
  t.assertEqual(result, 42);
  t.end();
});

asyncTest(
  "async function returning an object with primary async connstructor",
  async (t) => {
    const megaphone = await Megaphone.create();
    const result = await megaphone.fallibleMe(false);
    t.assertEqual(result, 42);
    t.end();
  },
);

asyncTest(
  "async function returning an object with secondary async connstructor",
  async (t) => {
    const megaphone = await Megaphone.secondary();

    const result = await megaphone.fallibleMe(false);
    t.assertEqual(result, 42);

    t.end();
  },
);

asyncTest("With the Tokio runtime", async (t) => {
  const helloAlice = await t.asyncMeasure(
    async () => sayAfterWithTokio(2000, "Alice"),
    2000,
    20,
  );
  t.assertEqual("Hello, Alice (with Tokio)!", helloAlice);
  t.end();
});

asyncTest("fallible function… which doesn't throw", async (t) => {
  const result = await t.asyncMeasure(async () => fallibleMe(false), 0, 100);
  t.assertEqual(42, result);
  t.end();
});

asyncTest("fallible method… which doesn't throw", async (t) => {
  const m = await fallibleStruct(false);
  const result = await m.fallibleMe(false);
  t.assertEqual(42, result);
  t.end();
});

asyncTest("fallible method… which doesn't throw, part II", async (t) => {
  const megaphone = newMegaphone();
  const result = await t.asyncMeasure(
    async () => megaphone.fallibleMe(false),
    0,
    100,
  );
  t.assertEqual(42, result);

  t.end();
});

asyncTest("fallible function… which does throw", async (t) => {
  await t.asyncMeasure(
    async () =>
      await t.assertThrowsAsync(MyError.Foo.instanceOf, async () =>
        fallibleMe(true),
      ),
    0,
    100,
  );
  t.end();
});

asyncTest("fallible method… which does throw", async (t) => {
  await t.assertThrowsAsync(MyError.Foo.instanceOf, async () =>
    fallibleStruct(true),
  );
  t.end();
});

asyncTest("fallible method… which does throw, part II", async (t) => {
  const megaphone = newMegaphone();
  await t.asyncMeasure(
    async () =>
      await t.assertThrowsAsync(MyError.Foo.instanceOf, async () =>
        megaphone.fallibleMe(true),
      ),
    0,
    100,
  );
  t.end();
});

asyncTest("a future that uses a lock and that is not cancelled", async (t) => {
  await useSharedResource(
    SharedResourceOptions.create({
      releaseAfterMs: 100,
      timeoutMs: 1000,
    }),
  );
  await useSharedResource(
    SharedResourceOptions.create({ releaseAfterMs: 0, timeoutMs: 1000 }),
  );
  t.end();
});

asyncTest("a future that uses a lock and that is cancelled", async (t) => {
  const options = SharedResourceOptions.create({
    releaseAfterMs: 100,
    timeoutMs: 1000,
  });
  const task = useSharedResource(options);
  await delayPromise(100);
  // Cancel the task
  // Unsure what this means in a Javascript context.
  await task;

  // Try accessing the shared resource again.  The initial task should release the shared resource
  // before the timeout expires.
  await useSharedResource(
    SharedResourceOptions.create({ releaseAfterMs: 0, timeoutMs: 1000 }),
  );
  t.end();
});

//
