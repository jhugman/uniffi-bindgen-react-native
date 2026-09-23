/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { emitJspiThunk, type WasmScalar } from "../src/jspi-thunk.js";

// Test binary encoding and frame layout without requiring JSPI. Suspension is
// covered by the generated bindings in integration/fixtures/jspi-wasm2-codegen.
for (const [type, value] of [
  ["i32", -2147483648],
  ["i64", -0x123456789abcdefn],
  ["f32", -1.25],
  ["f64", Math.PI],
] as const) {
  test(`thunk loads/stores ${type} across memory growth`, () => {
    const memory = new WebAssembly.Memory({ initial: 1 });
    const view = new DataView(memory.buffer);
    const frame = 128;
    if (type === "i32") view.setInt32(frame, value as number, true);
    if (type === "i64") view.setBigInt64(frame, value as bigint, true);
    if (type === "f32") view.setFloat32(frame, value as number, true);
    if (type === "f64") view.setFloat64(frame, value as number, true);
    view.setUint32(frame + 16, 0xdeadbeef, true);
    const mod = new WebAssembly.Module(emitJspiThunk([type], type));
    const thunk = new WebAssembly.Instance(mod, {
      env: {
        memory,
        target: (arg: unknown) => {
          assert.equal(arg, value);
          memory.grow(1);
          return arg;
        },
      },
    });
    (thunk.exports.call as Function)(frame);
    const result = new DataView(memory.buffer);
    if (type === "i32") assert.equal(result.getInt32(frame + 8, true), value);
    if (type === "i64")
      assert.equal(result.getBigInt64(frame + 8, true), value);
    if (type === "f32") assert.equal(result.getFloat32(frame + 8, true), value);
    if (type === "f64") assert.equal(result.getFloat64(frame + 8, true), value);
    assert.equal(result.getUint32(frame + 16, true), 0xdeadbeef);
  });
}

test("zero arguments and void need no result slot", () => {
  let called = 0;
  const memory = new WebAssembly.Memory({ initial: 0 });
  const thunk = new WebAssembly.Instance(
    new WebAssembly.Module(emitJspiThunk([])),
    {
      env: {
        memory,
        target: () => {
          called++;
        },
      },
    },
  );
  (thunk.exports.call as Function)(0);
  assert.equal(called, 1);
});

test("multi-byte section lengths and offsets preserve all arguments", () => {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const view = new DataView(memory.buffer);
  const values = Array.from({ length: 40 }, (_, i) => i - 20);
  values.forEach((v, i) => view.setInt32(128 + i * 8, v, true));
  const thunk = new WebAssembly.Instance(
    new WebAssembly.Module(
      emitJspiThunk(
        values.map(() => "i32"),
        "i32",
      ),
    ),
    {
      env: {
        memory,
        target: (...args: number[]) => {
          assert.deepEqual(args, values);
          return args.reduce((a, b) => a + b, 0);
        },
      },
    },
  );
  (thunk.exports.call as Function)(128);
  assert.equal(view.getInt32(128 + 40 * 8, true), -20);
});

test("invalid signature fails before emitting a module", () => {
  assert.throws(
    () => emitJspiThunk(["externref" as WasmScalar]),
    /Invalid WASM scalar/,
  );
  assert.throws(
    () => emitJspiThunk([], "toString" as WasmScalar),
    /Invalid WASM scalar/,
  );
});
