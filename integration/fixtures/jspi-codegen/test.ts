// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
import * as api from "./generated/ts/jspi_codegen";
import { uniffiRustFutureHandleCount } from "../../../typescript/src/async-rust-call";
import { UniffiInternalError } from "../../../typescript/src/errors";
import init from "./generated/ts/wasm-bindgen/index.js";

function check(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}

export async function run(bytes: BufferSource): Promise<string> {
  const pending = new Map<
    number,
    { resolve: (value: number) => void; reject: (error: Error) => void }
  >();
  (globalThis as any).jspiFixtureRequest = (id: number) =>
    new Promise<number>((resolve, reject) => {
      check(!pending.has(id), "duplicate pending import");
      pending.set(id, { resolve, reject });
    });
  const wasm = await init({ module_or_path: bytes });
  api.default.initialize();
  const settle = (id: number, value: number | Error) => {
    const request = pending.get(id);
    check(request, `missing import ${id}`);
    pending.delete(id);
    if (value instanceof Error) request.reject(value);
    else request.resolve(value);
  };
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
  let cases = 0;
  try {
    let firstDone = false;
    const first: Promise<number> = api.compute(1, 2).then((v) => {
      firstDone = true;
      return v;
    });
    const second: Promise<number> = api.compute(2, 1);
    await tick();
    check(
      !firstDone && Number(pending.size) === 2,
      "event loop progresses during real suspension",
    );
    const sync: number = api.synchronous(8);
    check(sync === 9, "unselected function stays synchronous");
    wasm.memory.grow(1);
    settle(2, 20);
    check((await second) === 20, "out of order scalar result");
    check(!firstDone, "first call still pending");
    settle(1, 10);
    await tick();
    check(pending.has(1) && !firstDone, "second suspension");
    settle(1, 11);
    check((await first) === 21, "stack preserved after overlap and growth");
    cases++;

    const byteResult: Promise<Uint8Array> = api.bytes(
      3,
      new Uint8Array([0, 128, 255]),
    );
    wasm.memory.grow(1);
    settle(3, 0);
    check(
      [...(await byteResult)].join() === "0,128,255",
      "byte buffer round trip",
    );
    cases++;

    const textResult: Promise<string> = api.text(4, "hello 🌍");
    settle(4, 0);
    check((await textResult) === "hello 🌍", "string lifting after suspension");
    cases++;

    const recordResult: Promise<api.Payload> = api.record(5, {
      label: "payload",
      value: 42,
    });
    settle(5, 0);
    const payload = await recordResult;
    check(
      payload.label === "payload" && payload.value === 42,
      "record round trip",
    );
    cases++;

    let voidDone = false;
    const voidResult: Promise<void> = api.completeVoid(6).then(() => {
      voidDone = true;
    });
    await tick();
    check(!voidDone, "void call must await its FFI Promise");
    settle(6, 0);
    check((await voidResult) === undefined && voidDone, "void completion");
    cases++;

    for (const call of [() => api.compute(7, 1), () => api.completeVoid(7)]) {
      const failure = call();
      settle(7, new Error("synthetic rejection"));
      let error: unknown;
      try {
        await failure;
      } catch (caught) {
        error = caught;
      }
      check(
        api.FixtureError.Rejected.instanceOf(error),
        "UniFFI error lifted after suspension",
      );
      cases++;
    }
    const plainScalar: Promise<number> = api.plainScalar(8);
    settle(8, 123);
    check((await plainScalar) === 123, "non-throwing scalar call");
    cases++;
    let plainVoidDone = false;
    const plainVoid: Promise<void> = api.plainVoid(9).then(() => {
      plainVoidDone = true;
    });
    await tick();
    check(!plainVoidDone, "non-throwing void call awaits settlement");
    settle(9, 0);
    await plainVoid;
    check(plainVoidDone, "non-throwing void call completes");
    cases++;

    const expectFixtureError = async (promise: Promise<unknown>) => {
      let error: unknown;
      try {
        await promise;
      } catch (caught) {
        error = caught;
      }
      check(
        api.FixtureError.Rejected.instanceOf(error),
        "lifted fixture error",
      );
    };
    const initialDrops: number = api.dropCount();
    let constructed = false;
    const construction = api.Processor.create(10, "primary").then((p) => {
      constructed = true;
      return p;
    });
    await tick();
    check(!constructed, "primary factory awaits Rust construction");
    settle(10, 0);
    const processor = await construction;
    check(
      api.Processor.instanceOf(processor),
      "primary factory returns concrete object",
    );
    cases++;

    const alternate = api.Processor.fromLabel(11, "alternate");
    settle(11, 0);
    const other = await alternate;
    check(
      api.Processor.instanceOf(other),
      "alternate factory returns concrete object",
    );
    cases++;

    for (const construct of [
      () => api.Processor.create(12, "failed"),
      () => api.Processor.fromLabel(12, "failed"),
    ]) {
      const before = api.dropCount();
      const failure = construct();
      settle(12, new Error("constructor rejected"));
      await expectFixtureError(failure);
      check(
        api.dropCount() === before + 1,
        "failed construction drops its partially built object",
      );
      cases++;
    }

    const display: Promise<string> = processor.asyncToString();
    settle(10, 0);
    check(
      (await display) === "Processor(primary)",
      "Display trait helper suspends",
    );
    cases++;

    for (const call of [() => processor.work(13), () => processor.touch(13)]) {
      const failedMethod = call();
      settle(13, new Error("method rejected"));
      await expectFixtureError(failedMethod);
      check(
        api.dropCount() === initialDrops + 2,
        "failed method only releases its cloned handle",
      );
      cases++;
    }
    let touchDone = false;
    const touched: Promise<void> = processor.plainTouch(14).then(() => {
      touchDone = true;
    });
    await tick();
    check(!touchDone, "non-throwing object void method awaits suspension");
    settle(14, 0);
    await touched;
    cases++;

    const firstWork: Promise<string> = processor.work(15);
    const secondWork: Promise<string> = processor.work(16);
    const beforeDestroy = api.dropCount();
    check(
      processor.uniffiDestroy() === undefined,
      "explicit destruction stays synchronous",
    );
    processor.uniffiDestroy();
    check(
      api.dropCount() === beforeDestroy,
      "in-flight clones keep the Rust object alive",
    );
    let destroyedError: unknown;
    try {
      await processor.work(99);
    } catch (error) {
      destroyedError = error;
    }
    check(
      destroyedError instanceof Error && !pending.has(99),
      "new calls reject after destruction",
    );
    wasm.memory.grow(1);
    settle(16, 16);
    check((await secondWork) === "primary:16", "second method completes first");
    check(
      api.dropCount() === beforeDestroy,
      "first method still holds the object",
    );
    settle(15, 15);
    check(
      (await firstWork) === "primary:15",
      "method completes after JS object destruction",
    );
    check(
      api.dropCount() === beforeDestroy + 1,
      "last in-flight clone releases Rust exactly once",
    );
    cases++;

    const partnerPromise = api.Processor.create(17, "partner");
    settle(17, 0);
    const partner = await partnerPromise;
    check(api.Processor.instanceOf(partner), "partner created");
    const joined: Promise<string> = other.join(partner, 18);
    const beforeJoin = api.dropCount();
    other.uniffiDestroy();
    partner.uniffiDestroy();
    check(
      api.dropCount() === beforeJoin,
      "receiver and object argument survive suspension",
    );
    settle(18, 0);
    check((await joined) === "alternate+partner", "object argument preserved");
    check(
      api.dropCount() === beforeJoin + 2,
      "both cloned objects released after method completion",
    );
    cases++;

    for (const reject of [false, true]) {
      const created = api.Processor.create(19, "scoped");
      settle(19, 0);
      const scoped = await created;
      check(api.Processor.instanceOf(scoped), "scoped object created");
      const before = api.dropCount();
      const result = scoped.uniffiUseAsync(async (obj) => {
        await obj.touch(20);
        return obj.work(21);
      });
      check(api.dropCount() === before, "async scope keeps object alive");
      settle(20, 0);
      await tick();
      check(
        pending.has(21) && api.dropCount() === before,
        "scope can use the object after an await",
      );
      settle(21, reject ? new Error("scope rejected") : 21);
      if (reject) await expectFixtureError(result);
      else
        check(
          (await result) === "scoped:21",
          "scope returns the awaited result",
        );
      check(
        api.dropCount() === before + 1,
        "scope releases object on either outcome",
      );
      cases++;
    }

    const original: api.Payload = api.Payload.create({
      label: "value",
      value: 42,
    });
    const delayedPayload: Promise<api.Payload> = api.Payload.delayed(
      original,
      22,
    );
    wasm.memory.grow(1);
    settle(22, 8);
    const changed = await delayedPayload;
    check(
      changed.value === 50 && original.value === 42,
      "record method lowers a snapshot and lifts its result after suspension",
    );
    cases++;
    const recordFailure: Promise<void> = api.Payload.touch(original, 23);
    settle(23, new Error("record rejected"));
    await expectFixtureError(recordFailure);
    cases++;
    let recordVoidDone = false;
    const recordVoid: Promise<void> = api.Payload.plainTouch(original, 24).then(
      () => {
        recordVoidDone = true;
      },
    );
    await tick();
    check(!recordVoidDone, "record void method awaits its Promise");
    settle(24, 0);
    await recordVoid;
    cases++;
    const mode: Promise<api.Mode> = api.Mode.delayed(api.Mode.Second, 25);
    settle(25, 0);
    check((await mode) === api.Mode.Second, "flat enum method suspends");
    cases++;
    const choice: Promise<api.Choice> = api.Choice.delayed(
      new api.Choice.Label({ value: "chosen" }),
      26,
    );
    settle(26, 0);
    const chosen = await choice;
    check(
      api.Choice.Label.instanceOf(chosen) && chosen.inner.value === "chosen",
      "tagged enum method suspends",
    );
    cases++;

    const syncObject = new api.SyncObject();
    const syncValue: number = syncObject.value();
    check(
      syncValue === 7,
      "unselected object constructor and methods stay synchronous",
    );
    syncObject.uniffiDestroy();
    cases++;

    // Real generated Rust async APIs use dedicated instrumented poll adapters.
    const asyncFirst: Promise<Uint8Array> = api.asyncBytes(40, true);
    const asyncSecond: Promise<Uint8Array> = api.asyncBytes(41, false);
    await tick();
    check(pending.has(40) && pending.has(41), "async polls genuinely suspend");
    wasm.memory.grow(1);
    settle(41, 41);
    check(
      new TextDecoder().decode(await asyncSecond) === "async:41",
      "owned async result",
    );
    settle(40, 40);
    check(
      new TextDecoder().decode(await asyncFirst) === "async:40",
      "wake and repoll after suspension",
    );
    cases++;

    const asyncFailure = api.asyncBytes(42, false);
    settle(42, new Error("async rejection"));
    await expectFixtureError(asyncFailure);
    cases++;

    const asyncVoid: Promise<void> = api.asyncVoid(43);
    settle(43, 0);
    await asyncVoid;
    cases++;

    const preAbort = new AbortController();
    preAbort.abort();
    check(
      (await api
        .asyncBytes(44, false, { signal: preAbort.signal })
        .catch((e) => e)) instanceof UniffiInternalError.AbortError,
      "pre-abort rejected",
    );
    check(!pending.has(44), "pre-abort never polls");
    cases++;

    const abort = new AbortController();
    let cancelledDone = false;
    const cancelled = api
      .asyncBytes(45, true, { signal: abort.signal })
      .catch((e) => {
        cancelledDone = true;
        return e;
      });
    abort.abort();
    await tick();
    check(!cancelledDone, "abort waits for active poll");
    settle(45, 45);
    check(
      (await cancelled) instanceof UniffiInternalError.AbortError,
      "Pending poll observes cancellation",
    );
    cases++;

    const racingAbort = new AbortController();
    const racing = api.asyncBytes(46, false, { signal: racingAbort.signal });
    racingAbort.abort();
    settle(46, 46);
    check(
      new TextDecoder().decode(await racing) === "async:46",
      "owned Ready result consumed despite cancellation race",
    );
    cases++;

    const asyncFactory: Promise<api.ProcessorLike> = api.Processor.asyncNew(
      47,
      "future",
    );
    settle(47, 0);
    const asyncObject = await asyncFactory;
    check(
      api.Processor.instanceOf(asyncObject),
      "async factory concrete instance",
    );
    const asyncDrops = api.dropCount();
    const asyncMethod: Promise<string> = asyncObject.asyncWork(48);
    asyncObject.uniffiDestroy();
    check(api.dropCount() === asyncDrops, "active future owns object");
    settle(48, 48);
    check((await asyncMethod) === "future:48", "async object method result");
    check(api.dropCount() === asyncDrops + 1, "future releases object");
    cases++;

    check(
      (await api.unselectedAsync(50)) === 51,
      "unselected Rust async still works",
    );
    const asyncRecord: Promise<api.Payload> = api.Payload.asyncCopy({
      label: "async record",
      value: 49,
    });
    settle(49, 0);
    check(
      (await asyncRecord).label === "async record",
      "async value receiver with no arguments",
    );
    cases++;
    check(
      uniffiRustFutureHandleCount() === 0,
      "no continuation handles remain",
    );
    cases++;

    if (false) {
      // @ts-expect-error A suspending constructor is exposed as the async static create factory.
      new api.Processor(0, "invalid");
    }
    check(pending.size === 0, "all imports settled");
    return `${cases} generated JSPI cases passed`;
  } finally {
    delete (globalThis as any).jspiFixtureRequest;
  }
}
