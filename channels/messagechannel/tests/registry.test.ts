/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { CallbackRegistry, ForwarderCache } from "../src/registry.js";
import type { CallbackPlan } from "../src/plan.js";

const PLAN: CallbackPlan = {
  name: "X",
  args: [],
  ret: { kind: "pass" },
  retTag: "Void",
  hasRustCallStatus: false,
  outReturn: false,
};

test("the same function registers once", () => {
  const r = new CallbackRegistry();
  const f = () => {};
  const g = () => {};
  const id = r.register(f, PLAN);
  assert.strictEqual(r.register(f, PLAN), id);
  assert.notStrictEqual(r.register(g, PLAN), id);
  assert.strictEqual(r.size, 2);
  assert.strictEqual(r.get(id)?.fn, f);
  assert.strictEqual(r.get(id)?.plan, PLAN);
});

test("ids start at 1 and never repeat after release", () => {
  const r = new CallbackRegistry();
  const a = r.register(() => {}, PLAN);
  assert.strictEqual(a, 1);
  r.release(a);
  assert.strictEqual(r.get(a), undefined);
  assert.strictEqual(r.size, 0);
  assert.strictEqual(
    r.register(() => {}, PLAN),
    2,
  );
});

test("releasing an unknown id is a no-op", () => {
  const r = new CallbackRegistry();
  r.release(99);
  assert.strictEqual(r.size, 0);
});

test("forwarder cache builds once per id", () => {
  const c = new ForwarderCache();
  let builds = 0;
  const f1 = c.forwarderFor(5, () => {
    builds++;
    return () => "built";
  });
  const f2 = c.forwarderFor(5, () => {
    builds++;
    return () => "again";
  });
  assert.strictEqual(f1, f2);
  assert.strictEqual(builds, 1);
  assert.strictEqual(c.size, 1);
  c.release(5);
  assert.strictEqual(c.size, 0);
  c.forwarderFor(5, () => () => "rebuilt");
  assert.strictEqual(builds, 1);
  assert.strictEqual(c.size, 1);
});
