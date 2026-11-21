/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The jsi player's stand-in for FinalizationRegistry, called directly, for
// what generated bindings do not exercise: tokens other than the target, a
// target in two registries, a cleanup that throws, and the TypeErrors.
import nativeModule from "@/generated/coverall-ffi";
import { asyncTest, type AsyncAsserts } from "@/asserts";
import "@/polyfills";

type Registry = {
  register(target: object, heldValue: unknown, unregisterToken?: object): void;
  unregister(unregisterToken: object): boolean;
};

const createFinalizationRegistry: (
  cleanup: (heldValue: any) => void,
) => Registry = (globalThis as any).uniffi.createFinalizationRegistry;

// The Hermes test-runner installs this; it forces a full collection.
const hermesGc: () => void = (globalThis as any).__hermesGc;

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

async function collectGarbage() {
  for (let i = 0; i < 20; i++) {
    hermesGc();
    await sleep(10);
  }
}

// Registers targets nothing else refers to, in a frame that is gone by the
// time the collection runs.
function registerDropped(
  registry: Registry,
  heldValues: unknown[],
  token?: object,
) {
  for (const heldValue of heldValues) {
    registry.register({}, heldValue, token);
  }
}

const isTypeError = (e: any) => e instanceof TypeError;

function sorted(values: unknown[]): string {
  return JSON.stringify([...values].sort());
}

(async () => {
  await asyncTest(
    "the cleanup gets each held value once its target is collected",
    async (t: AsyncAsserts) => {
      const cleaned: unknown[] = [];
      const registry = createFinalizationRegistry((h) => cleaned.push(h));
      registerDropped(registry, ["a", "b", "c"]);
      await collectGarbage();
      t.assertEqual(sorted(cleaned), sorted(["a", "b", "c"]));
      t.end();
    },
  );

  await asyncTest(
    "a token other than the target unregisters, once",
    async (t: AsyncAsserts) => {
      const cleaned: unknown[] = [];
      const registry = createFinalizationRegistry((h) => cleaned.push(h));
      const token = {};
      registerDropped(registry, ["x", "y"], token);
      t.assertTrue(registry.unregister(token), "the first unregister");
      t.assertFalse(registry.unregister(token), "the second unregister");
      t.assertFalse(registry.unregister({}), "a token never registered");
      await collectGarbage();
      t.assertEqual(sorted(cleaned), sorted([]));
      t.end();
    },
  );

  await asyncTest(
    "unregistering after the cleanup has run finds nothing",
    async (t: AsyncAsserts) => {
      const cleaned: unknown[] = [];
      const registry = createFinalizationRegistry((h) => cleaned.push(h));
      const token = {};
      registerDropped(registry, ["z"], token);
      await collectGarbage();
      t.assertEqual(sorted(cleaned), sorted(["z"]));
      t.assertFalse(registry.unregister(token));
      t.end();
    },
  );

  await asyncTest(
    "a target in two registries reaches both cleanups",
    async (t: AsyncAsserts) => {
      const cleaned: unknown[] = [];
      const first = createFinalizationRegistry((h) => cleaned.push(`1:${h}`));
      const second = createFinalizationRegistry((h) => cleaned.push(`2:${h}`));
      ((target: object) => {
        first.register(target, "t");
        second.register(target, "t");
      })({});
      await collectGarbage();
      t.assertEqual(sorted(cleaned), sorted(["1:t", "2:t"]));
      t.end();
    },
  );

  await asyncTest(
    "a cleanup that throws does not cost the others theirs",
    async (t: AsyncAsserts) => {
      const cleaned: unknown[] = [];
      const registry = createFinalizationRegistry((h) => {
        if (h === "bad") {
          throw new Error("expected: a cleanup that throws");
        }
        cleaned.push(h);
      });
      registerDropped(registry, ["good-1", "bad", "good-2"]);
      await collectGarbage();
      t.assertEqual(sorted(cleaned), sorted(["good-1", "good-2"]));
      t.end();
    },
  );

  await asyncTest("misuse is a TypeError", async (t: AsyncAsserts) => {
    t.assertThrows(isTypeError, () => createFinalizationRegistry(42 as any));
    const registry = createFinalizationRegistry(() => {});
    const target = {};
    t.assertThrows(isTypeError, () => registry.register(42 as any, "h"));
    t.assertThrows(isTypeError, () => registry.register(target, target));
    t.assertThrows(isTypeError, () =>
      registry.register(target, "h", 42 as any),
    );
    t.assertThrows(isTypeError, () => registry.unregister(42 as any));
    // The player's own buffers carry native state of their own, which a
    // destructor cannot share.
    const buffer = nativeModule().rustbuffer_alloc(8).buffer;
    t.assertThrows(isTypeError, () => registry.register(buffer, "h"));
    t.assertThrows(isTypeError, () => registry.register(target, "h", buffer));
    t.assertFalse(registry.unregister(buffer));
    t.end();
  });
})();
