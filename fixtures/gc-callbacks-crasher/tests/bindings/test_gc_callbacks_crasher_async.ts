/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-gc-callbacks-crasher -- async_wasm

import { asyncTest } from "@/asserts";
import {
  type AsyncDelegate,
  Builder,
  createArcDroppingBuilder,
  createDroppingBuilder,
  getBuilder,
} from "@/generated/gc_callbacks_crasher";

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

const asyncDelegate: AsyncDelegate = {
  method: async (_userId: string) => "foo",
};

(async () => {
  await asyncTest("using no callback", async (t) => {
    const makeClientBuilder = async () => {
      try {
        const builder = (await getBuilder()) as Builder;
        builder.uniffiDestroy();
      } catch (e) {
        console.error(e);
      }
    };
    for (let i = 0; i < 1000; i++) {
      await makeClientBuilder();
    }
    await delayPromise(1000);
    t.end();
  });
  await asyncTest("using uniffiDestroy", async (t) => {
    const makeClientBuilder = async () => {
      try {
        const builder = (await getBuilder()) as Builder;
        await builder.setAsyncDelegate(asyncDelegate);
        builder.uniffiDestroy();
      } catch (e) {
        console.error(e);
      }
    };
    for (let i = 0; i < 1000; i++) {
      await makeClientBuilder();
    }
    await delayPromise(1000);
    t.end();
  });

  await asyncTest("Dropping in Rust", async (t) => {
    try {
      for (let i = 0; i < 1000; i++) {
        await createDroppingBuilder(asyncDelegate);
      }
    } catch (e) {
      console.error(e);
    }
    await delayPromise(1000);
    t.end();
  });

  await asyncTest("Dropping Arc in Rust", async (t) => {
    try {
      for (let i = 0; i < 1000; i++) {
        await createArcDroppingBuilder(asyncDelegate);
      }
    } catch (e) {
      console.error(e);
    }
    await delayPromise(1000);
    t.end();
  });

  await asyncTest("using C++ destructors", async (t) => {
    const makeClientBuilder = async () => {
      try {
        const builder = (await getBuilder()) as Builder;
        await builder.setAsyncDelegate(asyncDelegate);
      } catch (e) {
        console.error(e);
      }
    };
    for (let i = 0; i < 1000; i++) {
      await makeClientBuilder();
    }
    await delayPromise(1000);
    t.end();
  });
})();
