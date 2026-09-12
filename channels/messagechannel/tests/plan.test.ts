/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { FfiType, type ModuleDefinitions } from "@ubjs/core";
import { compilePlan } from "../src/plan.js";

const SYMBOLS = {
  rustbuffer_alloc: "a",
  rustbuffer_free: "f",
  rustbuffer_from_bytes: "b",
};

function table(partial: Partial<ModuleDefinitions>): ModuleDefinitions {
  return {
    symbols: SYMBOLS,
    functions: {},
    callbacks: {},
    structs: {},
    ...partial,
  };
}

test("scalars, handles, buffers and void map to their plan kinds", () => {
  const plan = compilePlan(
    table({
      functions: {
        f: {
          args: [
            FfiType.UInt8,
            FfiType.Int64,
            FfiType.Float64,
            FfiType.Handle,
            FfiType.RustBuffer,
          ],
          ret: FfiType.Void,
          hasRustCallStatus: true,
        },
      },
    }),
  );
  const f = plan.functions.get("f")!;
  assert.deepStrictEqual(
    f.args.map((a) => a.kind),
    ["pass", "pass", "pass", "handle", "buffer"],
  );
  assert.deepStrictEqual(f.ret, { kind: "pass" });
  assert.strictEqual(f.hasRustCallStatus, true);
});

test("a callback passed to a function is persistent; one inside a callback is per invocation", () => {
  const plan = compilePlan(
    table({
      functions: {
        poll: {
          args: [FfiType.Handle, FfiType.Callback("Cont"), FfiType.Handle],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      callbacks: {
        Cont: {
          args: [FfiType.Handle, FfiType.Int8],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
        Method: {
          args: [FfiType.Handle, FfiType.Callback("Complete"), FfiType.Handle],
          ret: FfiType.Struct("Dropped"),
          hasRustCallStatus: false,
          outReturn: true,
        },
        Complete: {
          args: [FfiType.Handle],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
        DroppedFree: {
          args: [FfiType.Handle],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      structs: {
        Dropped: [
          { name: "handle", type: FfiType.Handle },
          { name: "free", type: FfiType.Callback("DroppedFree") },
        ],
      },
    }),
  );
  assert.deepStrictEqual(plan.functions.get("poll")!.args[1], {
    kind: "callback",
    name: "Cont",
    lifetime: "persistent",
  });
  const method = plan.callbacks.get("Method")!;
  assert.deepStrictEqual(method.args[1], {
    kind: "callback",
    name: "Complete",
    lifetime: "invocation",
  });
  assert.strictEqual(method.outReturn, true);
  assert.strictEqual(method.retTag, "Struct");
  assert.deepStrictEqual(method.ret, {
    kind: "struct",
    name: "Dropped",
    fields: [
      { name: "handle", plan: { kind: "handle" } },
      {
        name: "free",
        plan: { kind: "callback", name: "DroppedFree", lifetime: "invocation" },
      },
    ],
  });
});

test("Reference(Struct) compiles to the struct plan with persistent callbacks", () => {
  const plan = compilePlan(
    table({
      functions: {
        init: {
          args: [FfiType.Reference(FfiType.Struct("VT"))],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      callbacks: {
        Free: {
          args: [FfiType.Handle],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      structs: {
        VT: [{ name: "uniffi_free", type: FfiType.Callback("Free") }],
      },
    }),
  );
  assert.deepStrictEqual(plan.functions.get("init")!.args[0], {
    kind: "struct",
    name: "VT",
    fields: [
      {
        name: "uniffi_free",
        plan: { kind: "callback", name: "Free", lifetime: "persistent" },
      },
    ],
  });
});

test("RustCallStatus is a pass-through field but not an argument", () => {
  const ok = compilePlan(
    table({
      callbacks: {
        C: {
          args: [FfiType.Struct("R")],
          ret: FfiType.Void,
          hasRustCallStatus: false,
        },
      },
      structs: { R: [{ name: "call_status", type: FfiType.RustCallStatus }] },
    }),
  );
  assert.deepStrictEqual(ok.callbacks.get("C")!.args[0], {
    kind: "struct",
    name: "R",
    fields: [{ name: "call_status", plan: { kind: "pass" } }],
  });
  assert.throws(
    () =>
      compilePlan(
        table({
          functions: {
            f: {
              args: [FfiType.RustCallStatus],
              ret: FfiType.Void,
              hasRustCallStatus: false,
            },
          },
        }),
      ),
    /functions\.f arg 0: RustCallStatus/,
  );
});

test("unsupported and unknown tags are registration errors", () => {
  const cases: [t: any, re: RegExp][] = [
    [FfiType.VoidPointer, /functions\.f arg 0: VoidPointer/],
    [FfiType.ForeignBytes, /functions\.f arg 0: ForeignBytes/],
    [
      FfiType.Reference(FfiType.UInt8),
      /functions\.f arg 0: Reference to UInt8/,
    ],
    [
      FfiType.Callback("Missing"),
      /functions\.f arg 0: callback "Missing" is not in the table/,
    ],
    [
      FfiType.Struct("Missing"),
      /functions\.f arg 0: struct "Missing" is not in the table/,
    ],
    [{ tag: "Quaternion" }, /functions\.f arg 0: unknown tag "Quaternion"/],
  ];
  for (const [t, re] of cases) {
    assert.throws(
      () =>
        compilePlan(
          table({
            functions: {
              f: { args: [t], ret: FfiType.Void, hasRustCallStatus: false },
            },
          }),
        ),
      re,
      `expected ${JSON.stringify(t)} to be rejected`,
    );
  }
  assert.throws(
    () =>
      compilePlan(
        table({
          functions: {
            f: { args: [], ret: FfiType.VoidPointer, hasRustCallStatus: false },
          },
        }),
      ),
    /functions\.f ret: VoidPointer/,
  );
});

test("VoidPointer is accepted only as a vtable method's void out-return", () => {
  const plan = compilePlan(
    table({
      callbacks: {
        C: {
          args: [FfiType.Handle],
          ret: FfiType.VoidPointer,
          hasRustCallStatus: true,
          outReturn: true,
        },
      },
    }),
  );
  const c = plan.callbacks.get("C")!;
  assert.deepStrictEqual(c.ret, { kind: "pass" });
  assert.strictEqual(c.retTag, "VoidPointer");

  assert.throws(
    () =>
      compilePlan(
        table({
          callbacks: {
            C: {
              args: [FfiType.Handle],
              ret: FfiType.VoidPointer,
              hasRustCallStatus: true,
              outReturn: false,
            },
          },
        }),
      ),
    /callbacks\.C ret: VoidPointer/,
  );
  assert.throws(
    () =>
      compilePlan(
        table({
          callbacks: {
            C: {
              args: [FfiType.VoidPointer],
              ret: FfiType.Void,
              hasRustCallStatus: true,
              outReturn: true,
            },
          },
        }),
      ),
    /callbacks\.C arg 0: VoidPointer/,
  );
});
