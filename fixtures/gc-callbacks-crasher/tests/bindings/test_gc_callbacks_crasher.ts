/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { asyncTest } from "@/asserts";
import theModule, {
  type BasicDelegate,
  Builder,
  createArcDroppingBuilder,
  createDroppingBuilder,
  getBuilder,
} from "../../generated/gc_callbacks_crasher";

theModule.initialize();

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

const basicDelegate: BasicDelegate = {
  method: (userId: string) => {
    return "foo";
  },
};

(async () => {
  await asyncTest("using no callback", async (t) => {
    const makeClientBuilder = async () => {
      try {
        const builder = getBuilder();
        (builder as Builder).uniffiDestroy();
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
        const builder = getBuilder();
        builder.setBasicDelegate(basicDelegate);
        (builder as Builder).uniffiDestroy();
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
        createDroppingBuilder(basicDelegate);
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
        createArcDroppingBuilder(basicDelegate);
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
        const builder = getBuilder();
        builder.setBasicDelegate(basicDelegate);
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
