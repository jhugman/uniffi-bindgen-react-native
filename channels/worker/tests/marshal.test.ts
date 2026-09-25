/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import {
  marshalOut,
  marshalIn,
  wholeBuffer,
  type MarshalContext,
} from "../src/marshal.js";
import { CallbackRegistry, ForwarderCache } from "../src/registry.js";
import type { CallbackPlan, ValuePlan } from "../src/plan.js";

const CB: CallbackPlan = {
  name: "Cb",
  args: [{ kind: "handle" }],
  ret: { kind: "pass" },
  retTag: "Void",
  hasRustCallStatus: false,
  outReturn: false,
};

function ctx(
  over: Partial<MarshalContext> = {},
): MarshalContext & { built: number[] } {
  const built: number[] = [];
  return {
    built,
    registry: new CallbackRegistry(),
    forwarders: new ForwarderCache(),
    callbacks: new Map([["Cb", CB]]),
    makeForwarder: (id) => {
      built.push(id);
      return (..._a: unknown[]) => `fwd${id}`;
    },
    handleIn: (h) => h + 1000n,
    handleOut: (h) => h - 1000n,
    bufferOut: wholeBuffer,
    ...over,
  };
}

test("scalars pass through both ways", () => {
  const c = ctx();
  for (const v of [0, -1, 1.5, 2n ** 63n, undefined]) {
    assert.strictEqual(marshalOut({ kind: "pass" }, v, c, []), v);
    assert.strictEqual(marshalIn({ kind: "pass" }, v, c), v);
  }
});

test("handles go through the context's policy", () => {
  const c = ctx();
  assert.strictEqual(marshalIn({ kind: "handle" }, 5n, c), 1005n);
  assert.strictEqual(marshalOut({ kind: "handle" }, 1005n, c, []), 5n);
});

test("a whole-buffer view is transferred as is; a partial view is copied first", () => {
  const c = ctx();
  const whole = new Uint8Array([1, 2, 3]);
  const t: Transferable[] = [];
  assert.strictEqual(marshalOut({ kind: "buffer" }, whole, c, t), whole);
  assert.deepStrictEqual(t, [whole.buffer]);

  const backing = new Uint8Array([9, 1, 2, 3, 9]);
  const partial = backing.subarray(1, 4);
  const t2: Transferable[] = [];
  const out = marshalOut({ kind: "buffer" }, partial, c, t2) as Uint8Array;
  assert.notStrictEqual(out.buffer, backing.buffer);
  assert.deepStrictEqual(Array.from(out), [1, 2, 3]);
  assert.strictEqual(out.byteOffset, 0);
  assert.strictEqual(out.buffer.byteLength, 3);
  assert.deepStrictEqual(t2, [out.buffer]);
});

test("the same ArrayBuffer is listed for transfer once", () => {
  const c = ctx();
  const buf = new ArrayBuffer(4);
  const a = new Uint8Array(buf);
  const t: Transferable[] = [];
  marshalOut({ kind: "buffer" }, a, c, t);
  marshalOut({ kind: "buffer" }, a, c, t);
  assert.strictEqual(t.length, 1);
});

test("a function goes out as an id and comes in as one cached forwarder", () => {
  const out = ctx();
  const fn = () => {};
  const plan: ValuePlan = {
    kind: "callback",
    name: "Cb",
    lifetime: "persistent",
  };
  const id = marshalOut(plan, fn, out, []);
  assert.strictEqual(id, 1);
  assert.strictEqual(marshalOut(plan, fn, out, []), 1);
  assert.strictEqual(out.registry.get(1)?.plan, CB);

  const inn = ctx();
  const f1 = marshalIn(plan, 1, inn);
  const f2 = marshalIn(plan, 1, inn);
  assert.strictEqual(f1, f2);
  assert.deepStrictEqual(inn.built, [1]);
  assert.strictEqual((f1 as Function)(), "fwd1");
});

test("structs are walked field by field", () => {
  const c = ctx();
  const plan: ValuePlan = {
    kind: "struct",
    name: "S",
    fields: [
      { name: "handle", plan: { kind: "handle" } },
      {
        name: "free",
        plan: { kind: "callback", name: "Cb", lifetime: "invocation" },
      },
      { name: "bytes", plan: { kind: "buffer" } },
    ],
  };
  const free = () => {};
  const t: Transferable[] = [];
  const wire = marshalOut(
    plan,
    { handle: 1001n, free, bytes: new Uint8Array([7]) },
    c,
    t,
  ) as any;
  assert.strictEqual(wire.handle, 1n);
  assert.strictEqual(wire.free, 1);
  assert.deepStrictEqual(Array.from(wire.bytes), [7]);
  assert.strictEqual(t.length, 1);

  const back = marshalIn(
    plan,
    { handle: 1n, free: 1, bytes: new Uint8Array([7]) },
    c,
  ) as any;
  assert.strictEqual(back.handle, 1001n);
  assert.strictEqual(typeof back.free, "function");
});

test("type mismatches throw naming the plan", () => {
  const c = ctx();
  assert.throws(
    () =>
      marshalOut(
        { kind: "callback", name: "Cb", lifetime: "persistent" },
        42,
        c,
        [],
      ),
    /expected a function for callback "Cb"/,
  );
  assert.throws(
    () =>
      marshalIn(
        { kind: "callback", name: "Cb", lifetime: "persistent" },
        "x",
        c,
      ),
    /expected a callback id for "Cb"/,
  );
  assert.throws(
    () => marshalOut({ kind: "buffer" }, "nope", c, []),
    /expected a Uint8Array/,
  );
});
