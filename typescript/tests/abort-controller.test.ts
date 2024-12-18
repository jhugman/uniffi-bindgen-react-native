/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "../testing/polyfills";
import { test, asyncTest } from "../testing/asserts";

test("AbortController exists", (t) => {
  const controller = new AbortController();
  t.assertNotNull(controller);

  const signal = controller.signal;
  t.assertNotNull(signal);
});

(async () => {
  function delay(delayMs: number): Promise<void> {
    return new Promise<void>((resolve) => setTimeout(resolve, delayMs));
  }

  function cancellableDelay(
    delayMs: number,
    _props?: { signal: AbortSignal },
  ): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(resolve, delayMs);
      _props?.signal.addEventListener("abort", () => {
        // Stop the main operation
        clearTimeout(timer);
        // Reject the promise with the abort reason.
        reject(_props?.signal.reason);
      });
    });
  }

  await asyncTest("AbortController is polyfilled as expected", async (t) => {
    // This test passes if it AbortController and AbortSinal are available.
    let abortController = new AbortController();
    let abortSignal: AbortSignal = abortController.signal;

    // we don't care too much about the accuracy of the time here, just that the
    // classes exist and work as intended.
    await t.asyncMeasure(() => cancellableDelay(500), 500, 150);
    t.end();
  });

  await asyncTest("AbortController aborts as expected", async (t) => {
    const abortController = new AbortController();
    // A wait for an hour…
    const promise = cancellableDelay(60 * 60 * 1000, {
      signal: abortController.signal,
    });
    // …cancelled after 1/2 a second.
    const canceller = delay(500).then(() => abortController.abort());

    await Promise.allSettled([promise, canceller]);
    t.end();
  });

  await asyncTest("AbortController rejects with error", async (t) => {
    const abortController = new AbortController();

    // …cancelled after 1/2 a second.
    const canceller = delay(500).then(() => abortController.abort());

    try {
      await cancellableDelay(60 * 60 * 1000, {
        signal: abortController.signal,
      });
      t.fail("Expected an AbortError before we get here");
    } catch (err: any) {
      t.assertNotNull(err);
      t.assertTrue(err instanceof Error);
      t.assertEqual(err.name, "AbortError");
    }
    await Promise.allSettled([canceller]);
    t.end();
  });
})();
