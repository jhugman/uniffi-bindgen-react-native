/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import myModule, {
  asyncCallback,
  AsyncCallback,
  makeObject,
  makeObjectWithAsyncCallback,
  makeObjectWithCallback,
  simpleCallback,
  SimpleCallback,
  SimpleObject,
  throwObject,
} from "../../generated/wasm_arc_futures";
import { asyncTest, Asserts, test } from "@/asserts";
import {
  uniffiRustFutureHandleCount,
  uniffiForeignFutureHandleCount,
  UniffiThrownObject,
} from "uniffi-bindgen-react-native";
import "@/polyfills";

// Initialize the callbacks for the module.
// This will be hidden in the installation process.
myModule.initialize();

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
  await asyncTest(
    "Empty object; if this compiles, the test has passed",
    async (t) => {
      const obj = await makeObject();
      t.assertNotNull(obj);
      await obj.update("alice");
      // 200 ms later.
      checkRemainingFutures(t);
      t.end();
    },
  );

  await asyncTest("Updater actually calls the update callback", async (t) => {
    let previousValue: string | undefined;
    let updated = 0;
    class Updater implements SimpleCallback {
      onUpdate(old: string, new_: string): void {
        previousValue = old;
        updated++;
      }
      reset(): void {
        previousValue = undefined;
        updated = 0;
      }
    }

    const cbJs = new Updater();
    const cbRs = await simpleCallback(cbJs);
    cbRs.onUpdate("old", "new");
    t.assertEqual(previousValue, "old");
    t.assertEqual(updated, 1);
    cbJs.reset();

    const obj = await makeObjectWithCallback(cbJs);
    t.assertNotNull(obj);

    await obj.update("alice");
    t.assertEqual(previousValue, "key");
    t.assertEqual(updated, 1);

    await obj.update("bob");
    t.assertEqual(previousValue, "alice");
    t.assertEqual(updated, 2);

    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Updater actually calls the async callback", async (t) => {
    let updated = 0;
    let previousValue: string | undefined;
    class Updater implements AsyncCallback {
      async onUpdate(old: string, new_: string): Promise<void> {
        previousValue = old;
        updated++;
      }
      reset(): void {
        previousValue = undefined;
        updated = 0;
      }
    }

    const cbJs = new Updater();
    const cbRs = await asyncCallback(cbJs);
    await cbRs.onUpdate("old", "new");
    t.assertEqual(previousValue, "old");
    t.assertEqual(updated, 1);
    cbJs.reset();

    const obj = await makeObjectWithAsyncCallback(cbJs);
    t.assertNotNull(obj);

    await obj.update("alice");
    t.assertEqual(previousValue, "key");
    t.assertEqual(updated, 1);

    await obj.update("bob");
    t.assertEqual(previousValue, "alice");
    t.assertEqual(updated, 2);

    checkRemainingFutures(t);
    t.end();
  });

  await asyncTest("Object as error", async (t) => {
    await t.assertThrowsAsync((e: any) => {
      if (!UniffiThrownObject.instanceOf(e)) {
        return false;
      }
      if (!SimpleObject.hasInner(e)) {
        return false;
      }
      let obj = e.inner;
      t.assertNotNull(obj);
      t.assertTrue(SimpleObject.instanceOf(obj));
      return true;
    }, throwObject);
    checkRemainingFutures(t);
    t.end();
  });
})();
