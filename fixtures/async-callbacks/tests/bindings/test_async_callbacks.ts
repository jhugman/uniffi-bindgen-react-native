/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import myModule, {
  asStringUsingTrait,
  AsyncParser,
  cancelDelayUsingTrait,
  delayUsingTrait,
  ParserError,
  tryDelayUsingTrait,
  tryFromStringUsingTrait,
} from "../../generated/async_callbacks";
import { asyncTest, Asserts, test } from "@/asserts";
import {
  uniffiRustFutureHandleCount,
  uniffiForeignFutureHandleCount,
} from "uniffi-bindgen-react-native";
import "@/polyfills";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, delayMs));
}

function cancellableDelayPromise(
  delayMs: number,
  abortSignal: AbortSignal,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, delayMs);
    abortSignal.addEventListener("abort", () => {
      clearTimeout(timer);
      reject(abortSignal.reason);
    });
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

  class CancellableTsAsyncParser extends TsAsyncParser {
    /**
     * Each async callback method has an additional optional argument
     * `asyncOptions_`. This contains an `AbortSignal`.
     *
     * If the Rust task is cancelled, then this abort signal is
     * told, which can be used to co-operatively cancel the
     * async callback.
     *
     * @param delayMs
     * @param asyncOptions_
     */
    async delay(
      delayMs: number,
      asyncOptions_?: { signal: AbortSignal },
    ): Promise<void> {
      await this.doCancellableDelay(delayMs, asyncOptions_?.signal);
    }

    private async doCancellableDelay(
      ms: number,
      signal?: AbortSignal,
    ): Promise<void> {
      if (signal) {
        await cancellableDelayPromise(ms, signal);
      } else {
        await delayPromise(ms);
      }
      this.completedDelays += 1;
    }
  }

  /**
   * Rust supports task cancellation, but it's not automatic. It is rather like
   * Javascript's.
   *
   * In Javascript, an `AbortController` is used to make an `AbortSignal`.
   *
   * The task itself periodically checks the `AbortSignal` (or listens for an `abort` event),
   * then takes abortive actions. This usually happens when the `AbortController.abort` method
   * is called.
   *
   * In Rust, an `AbortHandle` is analagous to the `AbortController`.
   *
   * This test checks if that signal is being triggered by a Rust.
   */
  await asyncTest("cancellation of async JS callbacks", async (t) => {
    const traitObj = new CancellableTsAsyncParser();

    // #JS_TASK_CANCELLATION
    const completedDelaysBefore = traitObj.completedDelays;
    // This method calls into the async callback to sleep (in Javascript) for 100 seconds.
    // On a different thread, in Rust, it cancels the task. This sets the `AbortSignal` passed to the
    // callback function.
    await cancelDelayUsingTrait(traitObj, 10000);
    // If the task was cancelled, then completedDelays won't have increased.
    t.assertEqual(
      traitObj.completedDelays,
      completedDelaysBefore,
      "Delay should have been cancelled",
    );

    // Test that all handles here cleaned up
    checkRemainingFutures(t);
    t.end();
  });
})();
