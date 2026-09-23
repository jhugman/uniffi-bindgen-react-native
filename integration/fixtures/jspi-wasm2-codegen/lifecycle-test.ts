// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
import * as api from "./generated/api/index";
import { uniffiRustFutureHandleCount } from "../../../typescript/src/async-rust-call";

function check(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}

export async function run(bytes: Uint8Array): Promise<string> {
  const pending = new Map<
    number,
    { resolve: (value: number) => void; reject: (error: Error) => void }
  >();
  (globalThis as any).jspiFixtureRequest = (id: number) =>
    new Promise<number>((resolve, reject) => {
      check(!pending.has(id), "duplicate pending import");
      pending.set(id, { resolve, reject });
    });
  await api.uniffiInitAsync(bytes);
  const settle = (id: number, value: number | Error) => {
    const request = pending.get(id);
    check(request, `missing import ${id}`);
    pending.delete(id);
    if (value instanceof Error) request.reject(value);
    else request.resolve(value);
  };
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
  const grow = async () => {
    const growth = api.bytes(1000, new Uint8Array(8 * 1024 * 1024));
    settle(1000, 0);
    await growth;
  };
  let cases = 0;
  try {
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
    await grow();
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
    await grow();
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
    const failedAsyncFactory = api.Processor.asyncNew(51, "failed async");
    const failedDrops = api.dropCount();
    settle(51, new Error("async constructor failure"));
    await expectFixtureError(failedAsyncFactory);
    check(
      api.dropCount() === failedDrops + 1,
      "failed async factory drops partial object",
    );
    cases++;

    const abortedFactory = new AbortController();
    abortedFactory.abort();
    const beforePreabort = api.dropCount();
    try {
      await api.Processor.asyncNew(52, "preabort", {
        signal: abortedFactory.signal,
      });
      throw new Error("expected preabort");
    } catch (e) {
      check((e as Error).name === "AbortError", "factory preabort");
    }
    check(
      !pending.has(52) && api.dropCount() === beforePreabort,
      "preabort does not construct object",
    );
    cases++;

    const cancelCreate = api.Processor.create(53, "cancelled");
    settle(53, 0);
    const cancelObject = await cancelCreate;
    check(
      api.Processor.instanceOf(cancelObject),
      "cancellable concrete object",
    );
    const abort = new AbortController();
    const cancelling = cancelObject.asyncWork(54, { signal: abort.signal });
    const cancelDrops = api.dropCount();
    abort.abort();
    cancelObject.uniffiDestroy();
    await tick();
    check(
      api.dropCount() === cancelDrops,
      "aborted suspended method owns receiver",
    );
    settle(54, 54);
    check(
      (await cancelling) === "cancelled:54",
      "Ready method wins cancellation",
    );
    check(
      api.dropCount() === cancelDrops + 1,
      "cancelled method releases receiver after settlement",
    );
    cases++;

    const appended = api.Payload.append(
      { label: "receiver:", value: 7 },
      55,
      "x".repeat(8 * 1024 * 1024),
    );
    settle(55, 0);
    const appendedValue = await appended;
    check(
      appendedValue.label.length === 8 * 1024 * 1024 + 9 &&
        appendedValue.label.startsWith("receiver:") &&
        appendedValue.value === 7,
      "value receiver survives large argument allocation",
    );
    cases++;
    const asyncEnum = api.Choice.asyncDelayed(
      new api.Choice.Label({ value: "async enum" }),
      56,
    );
    settle(56, 0);
    const enumValue = await asyncEnum;
    check(
      api.Choice.Label.instanceOf(enumValue) &&
        enumValue.inner.value === "async enum",
      "async enum receiver",
    );
    cases++;
    check(
      uniffiRustFutureHandleCount() === 0,
      "all lifecycle future resolvers released",
    );
    const enumCopy = api.Choice.asyncCopy(enumValue);
    settle(57, 0);
    const copiedEnum = await enumCopy;
    check(
      api.Choice.Label.instanceOf(copiedEnum) &&
        copiedEnum.inner.value === "async enum",
      "no-argument async enum method",
    );
    cases++;
    check(pending.size === 0, "all lifecycle imports settled");
    return `${cases} generated wasm2 JSPI lifecycle cases passed`;
  } finally {
    delete (globalThis as any).jspiFixtureRequest;
  }
}
