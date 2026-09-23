/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { JspiCalls, isJspiResult } from "../src/jspi-call.js";
import { Memory } from "../src/memory.js";
import { Scratch } from "../src/scratch.js";
import { emitJspiThunk } from "../src/jspi-thunk.js";
import { writeRustBuffer } from "../src/marshal.js";
import type { DispatchContext, FunctionDef } from "../src/call.js";

// Mock only the promising entry, never trigger a real trap through Rust.
async function harness(
  body: (h: {
    ctx: DispatchContext;
    calls: JspiCalls;
    target: Function;
    frees: number[];
    setEntry: (fn: (frame: number) => Promise<void>) => void;
  }) => Promise<void>,
) {
  const promising = Object.getOwnPropertyDescriptor(WebAssembly, "promising");
  const suspending = Object.getOwnPropertyDescriptor(WebAssembly, "Suspending");
  let entry = async (_frame: number) => {};
  Object.defineProperty(WebAssembly, "promising", {
    configurable: true,
    value: () => (_slot: number, frame: number) => entry(frame),
  });
  Object.defineProperty(WebAssembly, "Suspending", {
    configurable: true,
    value: class {},
  });
  try {
    const memory = new WebAssembly.Memory({ initial: 1 });
    const table = new WebAssembly.Table({ initial: 0, element: "anyfunc" });
    const frees: number[] = [];
    let next = 1024;
    const ctx: DispatchContext = {
      memory: new Memory(memory),
      scratch: new Scratch(
        0,
        512,
        () => 0,
        () => {},
      ),
      structs: new Map(),
      callbackDefs: new Map(),
      useJit: false,
      alloc: (n, align) => {
        next = Math.ceil(next / align) * align;
        const p = next;
        next += n;
        return p;
      },
      free: (p) => {
        frees.push(p);
      },
      installCallback: () => {
        throw new Error("unexpected callback");
      },
    };
    // Real WASM export with (i32) -> void; used only for type-correct thunk
    // linking. The mock entry fills ABI storage instead of executing it.
    const target = new WebAssembly.Instance(
      new WebAssembly.Module(emitJspiThunk([])),
      { env: { memory, target: () => {} } },
    ).exports.call as Function;
    const calls = new JspiCalls({
      memory,
      __indirect_function_table: table,
      __ubrn_jspi_enter: target,
    });
    await body({
      ctx,
      calls,
      target,
      frees,
      setEntry: (fn) => {
        entry = fn;
      },
    });
  } finally {
    for (const [name, descriptor] of [
      ["promising", promising],
      ["Suspending", suspending],
    ] as const) {
      if (descriptor) Object.defineProperty(WebAssembly, name, descriptor);
      else Reflect.deleteProperty(WebAssembly, name);
    }
  }
}

test("JSPI retains frames on unexpected rejection and poisons subsequent calls", async () => {
  await harness(async ({ ctx, calls, target, frees, setEntry }) => {
    const cause = new Error("mock trap");
    setEntry(async () => {
      throw cause;
    });
    const def: FunctionDef = {
      args: [{ tag: "UInt32" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    };
    const call = calls.build(ctx, target, def);
    await assert.rejects(
      call(1),
      (e: Error) => e.name === "JspiCallError" && e.cause === cause,
    );
    assert.deepEqual(frees, []);
    assert.throws(() => calls.check(), /discard this WASM instance/);
    await assert.rejects(call(2), /discard this WASM instance/);
  });
});

test("JSPI frees preparation allocations when scalar conversion fails", async () => {
  await harness(async ({ ctx, calls, target, frees }) => {
    const call = calls.build(ctx, target, {
      args: [{ tag: "UInt32" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    });
    await assert.rejects(call(Symbol("bad")), TypeError);
    assert.deepEqual(frees, [1024]);
    calls.check();
  });
});

test("JSPI copies and frees owned result before resolution", async () => {
  await harness(async ({ ctx, calls, target, frees, setEntry }) => {
    // Target signature (i32) -> void: sret is the only argument.
    setEntry(async (frame) => {
      const sret = ctx.memory.readU32(frame);
      const ptr = ctx.alloc(3, 1);
      ctx.memory.view().set([1, 2, 3], ptr);
      writeRustBuffer(ctx.memory, sret, {
        capacity: 3n,
        len: 3n,
        dataPtr: ptr,
      });
    });
    const call = calls.build(ctx, target, {
      args: [],
      ret: { tag: "RustBuffer" },
      hasRustCallStatus: false,
    });
    const result = await call();
    assert.deepEqual([...result], [1, 2, 3]);
    assert.equal(isJspiResult(result), true);
    assert.equal(frees.length, 2); // payload and invocation frame
    assert.notEqual(result.buffer, ctx.memory.buffer());
  });
});

test("JSPI joins overlapping frames and releases in completion order", async () => {
  await harness(async ({ ctx, calls, target, frees, setEntry }) => {
    const pending: (() => void)[] = [];
    setEntry(() => new Promise<void>((resolve) => pending.push(resolve)));
    const call = calls.build(ctx, target, {
      args: [{ tag: "UInt32" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    });
    const a = call(1);
    const b = call(2);
    assert.deepEqual(frees, []);
    pending[1]();
    await b;
    assert.equal(frees.length, 1);
    assert.notEqual(frees[0], 1024);
    pending[0]();
    await a;
    assert.equal(frees[1], 1024);
  });
});

test("JSPI error status consumes its buffer and frame", async () => {
  await harness(async ({ ctx, calls, target, frees, setEntry }) => {
    setEntry(async (frame) => {
      const status = ctx.memory.readU32(frame);
      const payload = ctx.alloc(2, 1);
      ctx.memory.view().set([7, 8], payload);
      ctx.memory.writeU8(status, 1);
      writeRustBuffer(ctx.memory, status + 8, {
        capacity: 2n,
        len: 2n,
        dataPtr: payload,
      });
    });
    const status = { code: 0, errorBuf: new Uint8Array() };
    const call = calls.build(ctx, target, {
      args: [],
      ret: { tag: "Void" },
      hasRustCallStatus: true,
    });
    assert.equal(await call(status), undefined);
    assert.equal(status.code, 1);
    assert.deepEqual([...status.errorBuf], [7, 8]);
    assert.equal(frees.length, 2);
    calls.check();
  });
});

test("JSPI retains other in-flight frames after a fatal failure", async () => {
  await harness(async ({ ctx, calls, target, frees, setEntry }) => {
    const pending: { resolve: () => void; reject: (e: Error) => void }[] = [];
    setEntry(
      () =>
        new Promise<void>((resolve, reject) =>
          pending.push({ resolve, reject }),
        ),
    );
    const call = calls.build(ctx, target, {
      args: [{ tag: "UInt32" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    });
    const a = call(1);
    const b = call(2);
    pending[0].reject(new Error("mock trap"));
    await assert.rejects(a, /discard this WASM instance/);
    pending[1].resolve();
    await assert.rejects(b, /discard this WASM instance/);
    assert.deepEqual(frees, []);
  });
});

test("JSPI requires the instrumented entry", async () => {
  await harness(async () => {
    assert.throws(() => new JspiCalls({}), /export_jspi_entry/);
  });
});

test("JSPI lowers only registered synchronous future continuations", async () => {
  await harness(async ({ ctx, calls, target, setEntry }) => {
    const def: FunctionDef = {
      args: [{ tag: "Callback", name: "RustFutureContinuationCallback" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    };
    const call = calls.build(ctx, target, def);
    await assert.rejects(
      call(() => {}),
      /Invalid JSPI future continuation/,
    );
    const callback = () => {};
    ctx.callbackDefs.set("RustFutureContinuationCallback", {
      args: [{ tag: "Handle" }, { tag: "Int8" }],
      ret: { tag: "Void" },
      hasRustCallStatus: false,
    });
    ctx.installCallback = (fn) => {
      assert.equal(fn, callback);
      return 37;
    };
    setEntry(async (frame) => {
      assert.equal(ctx.memory.readU32(frame), 37);
    });
    await call(callback);
    assert.throws(
      () =>
        calls.build(ctx, target, {
          ...def,
          args: [{ tag: "Callback", name: "UserCallback" }],
        }),
      /only supports synchronous future continuations/,
    );
  });
});

test("synchronous completion ownership copies before freeing and survives growth", async () => {
  const { ownJspiResult } = await import("../src/jspi-call.js");
  const memory = new WebAssembly.Memory({ initial: 1 });
  const view = new Uint8Array(memory.buffer, 128, 4);
  view.set([1, 2, 3, 4]);
  let frees = 0;
  const copy = ownJspiResult(view, (v) => {
    assert.equal(v, view);
    frees++;
    memory.grow(1);
  });
  assert.equal(frees, 1);
  assert.equal(view.byteLength, 0);
  assert.deepEqual([...copy], [1, 2, 3, 4]);
  assert.ok(isJspiResult(copy));
});
