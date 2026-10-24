/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-async-channel -- async_wasm

import {
  Counter,
  type Greeter,
  RichError,
  ChannelError,
  checksum,
  echoBytes,
  echoSignal,
  failFlat,
  failLater,
  failRich,
  greetVia,
  liveCounters,
  liveFutures,
  makeLedger,
  sleepForever,
  sleepMs,
  total,
} from "@/generated/uniffi_async_channel";
import ffi, { setNativeModule } from "@/generated/uniffi_async_channel-ffi";
import { asyncTest } from "@/asserts";
import "@/polyfills";

// Every surface is a Promise under async delivery, including the constructor.
const mk = async (n: number): Promise<Counter> =>
  (await Counter.create(BigInt(n))) as Counter;

// Wraps the current module so every call reaching the player is counted by
// export name; `original` is what to restore afterwards.
function countingModule() {
  const original = ffi() as unknown as Record<string, Function>;
  const calls = new Map<string, number>();
  const nm = new Proxy(original, {
    get(target, key, receiver) {
      const v = Reflect.get(target, key, receiver);
      if (typeof key !== "string" || typeof v !== "function") return v;
      return (...args: unknown[]) => {
        calls.set(key, (calls.get(key) ?? 0) + 1);
        return v.apply(target, args);
      };
    },
  });
  return { original, nm, calls, reset: () => calls.clear() };
}

const totalCalls = (calls: Map<string, number>): number =>
  [...calls.values()].reduce((a, b) => a + b, 0);

(async () => {
  await asyncTest("methods and constructors resolve", async (t) => {
    const c = await mk(1);
    t.assertEqual(await c.value(), 1n);
    t.assertEqual(await c.add(41n), 42n);
    c.uniffiDestroy();
    t.end();
  });

  await asyncTest("one call carrying three clones of one handle", async (t) => {
    // sumWith(self, a, b) with the same object: the fire-and-forget clone
    // must land three times before the call consumes the handles.
    const c = await mk(5);
    t.assertEqual(await c.sumWith(c, c), 15n);
    const d = await mk(7);
    t.assertEqual(await c.sumWith(d, c), 17n);
    c.uniffiDestroy();
    d.uniffiDestroy();
    t.end();
  });

  await asyncTest(
    "objects inside a record, a list and an optional",
    async (t) => {
      const a = await mk(1);
      const b = await mk(2);
      t.assertEqual(
        await total({ name: "x", counters: [a, b], primary: a }),
        4n,
      );
      t.assertEqual(
        await total({ name: "x", counters: [], primary: undefined }),
        0n,
      );
      const ledger = await makeLedger("made", [10n, 20n]);
      t.assertEqual(ledger.counters.length, 2);
      t.assertEqual(await ledger.primary!.value(), 10n);
      t.assertEqual(await total(ledger), 40n);
      a.uniffiDestroy();
      b.uniffiDestroy();
      // The record's fields are CounterLike, which has no destructor.
      for (const c of ledger.counters) (c as Counter).uniffiDestroy();
      (ledger.primary as Counter).uniffiDestroy();
      t.end();
    },
  );

  await asyncTest("errors arrive as rejections", async (t) => {
    await t.assertThrowsAsync(
      // The variant carries its payload, so the string crossed the port intact.
      (e) => ChannelError.Flat.instanceOf(e) && e.inner[0] === "nope",
      () => failFlat("nope"),
    );
    await t.assertThrowsAsync(
      (e) => RichError.hasInner(e) && RichError.getInner(e) !== undefined,
      () => failRich(7),
    );
    try {
      await failRich(9);
    } catch (e: any) {
      t.assertEqual(await RichError.getInner(e).code(), 9);
    }
    t.end();
  });

  await asyncTest("a large buffer round-trips intact", async (t) => {
    const n = 300 * 1024;
    const bytes = new Uint8Array(n);
    for (let i = 0; i < n; i++) bytes[i] = i % 251;
    let expected = 0n;
    for (let i = 0; i < n; i++) expected += BigInt(i % 251);
    const back = await echoBytes(bytes.buffer.slice(0));
    t.assertEqual(back.byteLength, n);
    t.assertEqual(await checksum(back), expected);
    // Only the lowered RustBuffer is transferred, so `back` survives being passed on.
    t.assertEqual(new Uint8Array(back)[n - 1], (n - 1) % 251);
    t.end();
  });

  await asyncTest(
    "an async callback interface round-trips over the port",
    async (t) => {
      class TsGreeter implements Greeter {
        async greet(name: string): Promise<string> {
          await new Promise((r) => setTimeout(r, 5));
          return `hi ${name}`;
        }
      }
      t.assertEqual(await greetVia(new TsGreeter(), "chan"), "hi chan");
      t.end();
    },
  );

  await asyncTest(
    "the worker drives the future: one player call per await",
    async (t) => {
      const { original, nm, calls, reset } = countingModule();
      setNativeModule(nm as any);
      try {
        t.assertEqual(await sleepMs(20), 20);
        t.assertEqual(totalCalls(calls), 1);
        t.assertEqual(
          calls.get("uniffi_uniffi_async_channel_fn_func_sleep_ms"),
          1,
        );

        reset();
        await t.assertThrowsAsync(
          (e) => ChannelError.Flat.instanceOf(e),
          () => failLater(5),
        );
        t.assertEqual(totalCalls(calls), 1);

        reset();
        const controller = new AbortController();
        const p = sleepForever({ signal: controller.signal });
        setTimeout(() => controller.abort(), 20);
        await t.assertThrowsAsync(
          (e) => e instanceof Error && e.name === "AbortError",
          () => p,
        );
        // The cancel crosses as a message, not a player call.
        t.assertEqual(totalCalls(calls), 1);

        const c = await mk(3);
        reset();
        t.assertEqual(await c.valueAfter(5), 3n);
        t.assertEqual(
          calls.get(
            "uniffi_uniffi_async_channel_fn_method_counter_value_after",
          ),
          1,
        );
        // The entry, plus the receiver's clone. Counted before the destroy,
        // whose free also reaches the player.
        t.assertEqual(totalCalls(calls), 2);
        c.uniffiDestroy();
      } finally {
        setNativeModule(original as any);
      }
      t.end();
    },
  );

  await asyncTest(
    "nothing is left behind after many awaits, cancelled and errored ones included",
    async (t) => {
      for (let i = 0; i < 20; i++) {
        await sleepMs(1);
        await t.assertThrowsAsync(
          (e) => ChannelError.Flat.instanceOf(e),
          () => failLater(1),
        );
        const controller = new AbortController();
        const p = sleepForever({ signal: controller.signal });
        setTimeout(() => controller.abort(), 1);
        await t.assertThrowsAsync(
          (e) => e instanceof Error && e.name === "AbortError",
          () => p,
        );
        const c = await mk(i);
        await c.valueAfter(1);
        c.uniffiDestroy();
      }
      // Every future was freed in the worker, not just settled on the page.
      t.assertEqual(await liveFutures(), 0);
      t.assertEqual(await liveCounters(), 0);
      t.end();
    },
  );

  await asyncTest(
    "an argument named like the entry's signal reaches Rust",
    async (t) => {
      t.assertEqual(await echoSignal(7), 7);
      t.end();
    },
  );

  await asyncTest("async Rust resolves, and cancels mid-flight", async (t) => {
    t.assertEqual(await sleepMs(20), 20);
    const controller = new AbortController();
    const p = sleepForever({ signal: controller.signal });
    setTimeout(() => controller.abort(), 20);
    await t.assertThrowsAsync(
      (e) => e instanceof Error && e.name === "AbortError",
      () => p,
    );
    t.end();
  });

  await asyncTest("every counter is freed", async (t) => {
    // Frees are fire-and-forget; a round trip through Rust orders after them.
    t.assertEqual(await liveCounters(), 0);
    t.end();
  });
})();
