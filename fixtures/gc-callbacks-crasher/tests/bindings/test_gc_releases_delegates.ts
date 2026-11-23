/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// A Rust object that JS drops is freed after a garbage collection, even when
// freeing it calls back into JS: dropping the Builder drops the JS delegate it
// holds, and only that callback lets JS release the delegate.
//
// A WeakRef to each delegate shows whether JS has released it.
import { asyncTest } from "@/asserts";
import { collectGarbage } from "@/gc";
import theModule, { getBuilder } from "@/generated/gc_callbacks_crasher";

theModule.initialize();

const N = 10;

// In a function of its own, so that neither the builders nor the delegate are
// still on the stack when the collection runs.
function dropBuilderHoldingDelegate(): WeakRef<object> {
  const delegate = { method: async (_userId: string) => "foo" };
  getBuilder().setAsyncDelegate(delegate);
  return new WeakRef(delegate);
}

(async () => {
  await asyncTest(
    "delegates held by dropped Rust objects are released after a GC",
    async (t) => {
      const delegates = Array.from({ length: N }, dropBuilderHoldingDelegate);
      // Collecting the Builders frees them in Rust, which releases the
      // delegates; only a later collection can then find the delegates.
      await collectGarbage();
      const held = delegates.filter((d) => d.deref() !== undefined).length;
      t.assertEqual(held, 0, `${held} of ${N} delegates still held`);
      t.end();
    },
  );
})();
