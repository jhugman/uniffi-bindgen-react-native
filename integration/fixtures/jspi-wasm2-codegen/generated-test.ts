// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
import * as api from "./generated/api/index";

function check(value: unknown, message: string) {
  if (!value) throw new Error(message);
}
export async function run(bytes: Uint8Array) {
  const pending = new Map<
    number,
    { resolve: (v: number) => void; reject: (e: Error) => void }
  >();
  const globals = globalThis as typeof globalThis & {
    jspiFixtureRequest?: (id: number) => Promise<number>;
  };
  globals.jspiFixtureRequest = (id) =>
    new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  function settle(id: number, value: number | Error) {
    const request = pending.get(id);
    if (!request) throw new Error(`request ${id} missing`);
    pending.delete(id);
    if (value instanceof Error) request.reject(value);
    else request.resolve(value);
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
  try {
    await api.uniffiInitAsync(bytes);
    const first = api.compute(1, 2);
    const second = api.compute(2, 1);
    check(
      first instanceof Promise && second instanceof Promise,
      "selected functions return promises",
    );
    await tick();
    check(
      api.synchronous(5) === 6 && pending.size === 2,
      "synchronous API during suspension",
    );
    settle(2, 20);
    check(
      new TextDecoder().decode(await second) === "2:20",
      "out of order completion",
    );
    settle(1, 10);
    await tick();
    settle(1, 11);
    check(
      new TextDecoder().decode(await first) === "1:21",
      "repeated suspension",
    );

    const rejected = api.compute(3, 1);
    settle(3, new Error("synthetic"));
    let caught = false;
    try {
      await rejected;
    } catch (error) {
      caught = api.FixtureError.instanceOf(error);
    }
    check(caught, "generated UniFFI error lifting");

    const mixed = api.mixed(4, -1234567890123n, 0.5, 0.25);
    const wide = api.wide(5, 0xfedcba9876543210n);
    const narrow = api.narrow(6, -1.25);
    const scalar = api.scalar(7, -123456789);
    const voidCall = api.completeVoid(8);
    for (const id of [8, 7, 6, 5, 4]) settle(id, 0);
    check((await mixed) === -1234567890123 * 0.5 + 0.25, "mixed types");
    check((await wide) === 0xfedcba9876543210n, "wide return");
    check(
      (await narrow) === -1.25 && (await scalar) === -123456789,
      "scalar return",
    );
    check((await voidCall) === undefined, "void return");

    const data = new Uint8Array(1024 * 1024).fill(97);
    const owned1 = api.bytes(9, data);
    const owned2 = api.bytes(10, data);
    settle(10, 0);
    settle(9, 0);
    const [a, b] = await Promise.all([owned1, owned2]);
    check(
      a.length === data.length &&
        b.length === data.length &&
        a[0] === 97 &&
        b.at(-1) === 97,
      "owned input/output survives overlap and growth",
    );
    // A later allocation must not detach already returned JS-owned results.
    const later = api.bytes(11, new Uint8Array(4 * 1024 * 1024).fill(98));
    settle(11, 0);
    await later;
    check(
      a.byteLength === data.length && a[0] === 97,
      "returned buffers stay usable",
    );
    const ownedError = api.bytes(12, data);
    settle(12, new Error("synthetic"));
    caught = false;
    try {
      await ownedError;
    } catch (error) {
      caught = api.FixtureError.instanceOf(error);
    }
    check(caught, "owned input on error");
    const combined = api.combine(
      13,
      data,
      new Uint8Array(4 * 1024 * 1024).fill(98),
    );
    settle(13, 0);
    const c = await combined;
    check(
      c.length === 5 * 1024 * 1024 && c[0] === 97 && c[data.length] === 98,
      "multiple owned arguments survive allocation growth",
    );
    const text = api.text(14, "suspending λ");
    settle(14, 0);
    check((await text) === "suspending λ", "string round trip");
    const record = api.record(15, { label: "record", value: 42 });
    settle(15, 0);
    const r = await record;
    check(r.label === "record" && r.value === 42, "record round trip");
    const noArgs = api.noArguments();
    settle(99, 123);
    check((await noArgs) === 123, "nonthrowing no-argument call");
    const voidError = api.completeVoid(16);
    settle(16, new Error("void"));
    caught = false;
    try {
      await voidError;
    } catch (error) {
      caught = api.FixtureError.instanceOf(error);
    }
    check(caught, "void error status consumed after settlement");
    check(
      (await api.ordinaryFuture(41)) === 42,
      "unselected Rust async API unchanged",
    );
    const dropsBefore = api.selectedFutureDrops();
    const afirst = api.suspendedFuture(20, false);
    const asecond = api.suspendedFuture(21, false);
    check(api.synchronous(1) === 2, "sync calls during async poll");
    check(
      (await api.ordinaryBytes())[0] === 42 &&
        (await api.ordinaryFuture(4)) === 5,
      "unselected async calls share the same poll types safely",
    );
    settle(21, 21);
    settle(20, 20);
    const [aa, ab] = await Promise.all([afirst, asecond]);
    check(
      aa.length === 1024 * 1024 && aa[0] === 20 && ab.at(-1) === 21,
      "async owned results during growth",
    );
    const waiting = api.suspendedFuture(22, true);
    settle(22, 22);
    await tick();
    api.wakeSelectedFuture(22);
    check((await waiting)[0] === 22, "async Pending wake repoll");
    async function expectAbort(p: Promise<unknown>) {
      try {
        await p;
        throw new Error("expected abort");
      } catch (e) {
        check(
          (e as Error).name === "AbortError",
          `unexpected abort error ${e}`,
        );
      }
    }
    const pre = new AbortController();
    pre.abort();
    await expectAbort(api.suspendedFuture(23, false, { signal: pre.signal }));
    check(!pending.has(23), "preabort skips Rust");
    const readyAbort = new AbortController();
    const ready = api.suspendedFuture(24, false, { signal: readyAbort.signal });
    const d = api.selectedFutureDrops();
    readyAbort.abort();
    await tick();
    check(
      api.selectedFutureDrops() === d,
      "suspended future retained until settlement",
    );
    settle(24, 24);
    check((await ready)[0] === 24, "Ready beats cancellation");
    const during = new AbortController();
    const cancelled = api.suspendedFuture(25, true, { signal: during.signal });
    during.abort();
    settle(25, 25);
    await expectAbort(cancelled);
    const after = new AbortController();
    const late = api.suspendedFuture(26, true, { signal: after.signal });
    settle(26, 26);
    await tick();
    after.abort();
    await expectAbort(late);
    const failed = api.suspendedFuture(27, false);
    settle(27, new Error("async failure"));
    caught = false;
    try {
      await failed;
    } catch (e) {
      caught = api.FixtureError.instanceOf(e);
    }
    check(
      caught && api.selectedFutureDrops() === dropsBefore + 7,
      "async errors and exact guard drops",
    );
    const joined = api.asyncCombine(
      28,
      data,
      new Uint8Array(8 * 1024 * 1024).fill(99),
    );
    settle(28, 0);
    const joinedBytes = await joined;
    check(
      joinedBytes.length === 9 * 1024 * 1024 &&
        joinedBytes[0] === 97 &&
        joinedBytes.at(-1) === 99,
      "multiple async creation buffers survive lowering growth",
    );
    check(
      aa[0] === 20 && ab.at(-1) === 21,
      "async copied results remain stable",
    );
    const scalarFuture = api.asyncScalar(29),
      voidFuture = api.asyncVoid(30);
    settle(29, 123);
    settle(30, 0);
    check(
      (await scalarFuture) === 123 && (await voidFuture) === undefined,
      "async scalar and void completion",
    );
    const voidFailure = api.asyncVoid(31);
    settle(31, new Error("async void failure"));
    caught = false;
    try {
      await voidFailure;
    } catch (e) {
      caught = api.FixtureError.instanceOf(e);
    }
    check(caught, "async void error");
    check(pending.size === 0, "all generated calls settled");
    return "28 generated wasm2 JSPI cases passed";
  } finally {
    delete globals.jspiFixtureRequest;
  }
}
