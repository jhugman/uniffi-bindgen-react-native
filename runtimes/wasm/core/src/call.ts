/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { Memory } from "./memory.js";
import type { Scratch } from "./scratch.js";
import {
  FOREIGN_BYTES_SIZE,
  RUST_CALL_STATUS_SIZE,
  RUST_BUFFER_SIZE,
  RCS_ERROR_BUF_OFF,
  readRustBuffer,
  writeForeignBytes,
  writeRustBufferPayload,
  writeRustCallStatusZero,
  type RustBufferLike,
  type StructLayout,
} from "./marshal.js";
import type { FfiTypeDesc } from "./ffi-type.js";
import type { CallbackDef } from "./callback.js";

export interface FunctionDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
}

export interface DispatchContext {
  memory: Memory;
  scratch: Scratch;
  structs: Map<string, StructLayout>;
  // Per-shape definitions, keyed by the same name codegen passes in
  // `FfiType.Callback("...")`. Used by `planArg` to recover the trampoline
  // signature when lowering JS functions.
  callbackDefs: Map<string, CallbackDef>;
  // Wasm-side allocator for byte buffers: RustBuffer args and returns, plus
  // errorBuf inside RustCallStatus. The helper crate's
  // `__ubrn_alloc`/`__ubrn_free` reach the same global allocator Rust uses
  // for `Vec<u8>`, so a buffer allocated here with `(size, align=1)` can be
  // dropped on the Rust side via `Vec::from_raw_parts`, and vice versa.
  alloc: (size: number, align: number) => number;
  free: (ptr: number, size: number, align: number) => void;
  // Install a JS function as a wasm callback (per-closure trampoline + table
  // slot). Returns the table index that Rust's `call_indirect` will land on.
  installCallback: (fn: (...args: any[]) => any, def: CallbackDef) => number;
  // Whether the dispatcher may JIT-compile per-export bodies via
  // `new Function`. `module.ts` decides once at register time; when false,
  // `specializeFunction` always takes the interpreted path.
  useJit: boolean;
}

// How much of the reserved scratch region a value needs. This is all
// `computeAndReserveLayout` looks at, so the JIT path can lay a call out
// without building the `prepare`/`finish` closures it never calls.
interface ScratchSlot {
  scratchSize: number; // 0 for pass-by-value scalars
  scratchAlign: number;
}

// What `prepare` returns for one argument. Most args are a bare wasm-arg word
// (pointer or scalar). An arg that owns transient wasm storage returns the
// word plus a cleanup closure that releases it after the call — on the error
// path too, because the callee only *borrows* the bytes (`ForeignBytes`).
//
// The closure must capture this call's allocation rather than re-reading the
// shared reserved scratch slot: a re-entrant call of the same export overwrites
// that slot before the outer `finally` runs, so re-reading it would free the
// inner allocation twice and leak the outer one.
interface PreparedArg {
  word: number;
  cleanup?: () => void;
}

interface ArgPlan extends ScratchSlot {
  // What the dispatcher does at call-time per arg. Scalars are a `number`
  // (i32/f32/f64) or a `bigint` (i64/handle); args owning transient storage
  // return a `PreparedArg`.
  prepare: (
    ctx: DispatchContext,
    base: number,
    jsArg: any,
  ) => number | bigint | PreparedArg;
}

interface RetSlot extends ScratchSlot {
  hasSret: boolean;
}

interface RetPlan extends RetSlot {
  finish: (ctx: DispatchContext, sretBase: number, scalarRet: any) => any;
}

const SCALAR_SLOT: ScratchSlot = { scratchSize: 0, scratchAlign: 1 };
const RUST_BUFFER_SLOT: ScratchSlot = {
  scratchSize: RUST_BUFFER_SIZE,
  scratchAlign: 8,
};

const FOREIGN_BYTES_SLOT: ScratchSlot = {
  scratchSize: FOREIGN_BYTES_SIZE,
  scratchAlign: 4,
};

/** Scratch a lowered arg needs. `RustBuffer` and `ForeignBytes` are structs
 * written into a slot; everything else — scalars, callback table indices, and
 * the persistently-allocated `Reference(Struct)` — passes by value. */
function argSlot(t: FfiTypeDesc): ScratchSlot {
  switch (t.tag) {
    case "RustBuffer":
      return RUST_BUFFER_SLOT;
    case "ForeignBytes":
      return FOREIGN_BYTES_SLOT;
    default:
      return SCALAR_SLOT;
  }
}

/** Scratch a return value needs. `RustBuffer` returns come back through an
 * sret pointer; scalars and `Void` come back in the wasm return slot. */
function retSlot(t: FfiTypeDesc): RetSlot {
  return t.tag === "RustBuffer"
    ? { hasSret: true, ...RUST_BUFFER_SLOT }
    : { hasSret: false, ...SCALAR_SLOT };
}

const align = (off: number, a: number) => (off + (a - 1)) & ~(a - 1);

/**
 * Probe for `new Function` availability. Returns true if we can JIT-compile
 * dispatchers; false under strict CSP. The probe is cheap (one Function
 * construction) and the result is captured at register() time for the
 * lifetime of the module.
 */
export function canUseFunctionConstructor(): boolean {
  try {
    // eslint-disable-next-line no-new-func
    return new Function("return 42")() === 42;
  } catch {
    return false;
  }
}

function planArg(t: FfiTypeDesc): ArgPlan {
  switch (t.tag) {
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "VoidPointer":
    case "MutReference":
      return { ...argSlot(t), prepare: (_c, _b, v) => v | 0 };
    case "UInt64":
    case "Int64":
    case "Handle":
      return { ...argSlot(t), prepare: (_c, _b, v) => v }; // bigint passes through
    case "Float32":
    case "Float64":
      return { ...argSlot(t), prepare: (_c, _b, v) => v };
    case "RustBuffer":
      return {
        ...argSlot(t),
        // Codegen lowers every `RustBuffer`-shaped arg via
        // `converter.lower(value, rustbuffer_alloc)`, so the view arriving
        // here already aliases wasm memory: its `byteOffset` is the
        // `dataPtr`. All that remains is the (capacity, len, dataPtr) struct
        // Rust reads as a `RustBuffer`. Rust drops it via
        // `Vec::from_raw_parts`, so don't free `dataPtr` here.
        prepare: (ctx, base, v: Uint8Array) => {
          writeRustBufferPayload(ctx.memory, ctx.alloc, base, v);
          return base;
        },
      };
    case "ForeignBytes":
      // `&[u8]` arguments. Codegen lowers these via
      // `converter.lowerBorrowed(value)`, which hands us the caller's JS-owned
      // `Uint8Array` — a `subarray` stays a view, so only its
      // `byteOffset .. byteOffset + byteLength` window is meaningful, which
      // `Memory.writeBytes` honours. Rust's `ForeignBytes` is
      // `{ len: i32, data: *const u8 }`; wasm32 passes the 8-byte struct by
      // pointer (unlike the native players' libffi, which passes it by value),
      // so write it into the reserved slot and pass the slot's address.
      //
      // Unlike `RustBuffer`, Rust only *borrows* the payload for the duration
      // of the call and never frees it. The copy is therefore live wasm memory
      // we own, and `cleanup` releases it afterwards.
      return {
        ...argSlot(t),
        prepare: (ctx, base, v: Uint8Array) => {
          // Codegen hands us a `Uint8Array`; anything else would silently copy
          // nothing (or throw a bare `TypeError` from `set`) *after* an
          // allocation. `instanceof` matches the check `module.ts` already uses
          // for byte sources.
          if (!(v instanceof Uint8Array)) {
            throw new Error(
              `planArg(ForeignBytes): expected a Uint8Array, got ${Object.prototype.toString.call(v)}`,
            );
          }
          const len = v.byteLength;
          // A detached view reports length 0, so without this it would decay
          // into a `(len 0, null)` argument and the caller's bytes would
          // vanish. Prefer the precise `ArrayBuffer.prototype.detached` signal;
          // fall back to `writeRustBufferPayload`'s heuristic (a live buffer
          // cannot be shorter than one of its views).
          const buf = v.buffer as ArrayBuffer & { detached?: boolean };
          if (buf.detached === true || (len > 0 && buf.byteLength === 0)) {
            throw new Error(
              "planArg(ForeignBytes): source view is detached — cannot copy " +
                "it into wasm memory. The view must not be held across any " +
                "operation that can grow wasm memory.",
            );
          }
          // Zero-length is legal and must reach Rust as `(len = 0, data =
          // null)`: `ForeignBytes::as_slice` asserts a null pointer only pairs
          // with len 0, and a non-null pointer with len 0 is still fine, but
          // avoiding the alloc keeps it free of side effects.
          let dataPtr = 0;
          if (len > 0) {
            dataPtr = ctx.alloc(len, 1);
            ctx.memory.writeBytes(dataPtr, v);
          }
          writeForeignBytes(ctx.memory, base, { len, dataPtr });
          // Capture this call's `dataPtr`/`len` (not the scratch slot) so a
          // re-entrant call cannot make this cleanup free its storage.
          const cleanup =
            dataPtr !== 0 ? () => ctx.free(dataPtr, len, 1) : undefined;
          return { word: base, cleanup };
        },
      };
    case "Callback": {
      const name = t.name;
      return {
        ...argSlot(t),
        prepare: (ctx, _base, fn: (...args: any[]) => any) => {
          const def = ctx.callbackDefs.get(name);
          if (!def) {
            throw new Error(`planArg: callback shape "${name}" not registered`);
          }
          return ctx.installCallback(fn, def);
        },
      };
    }
    case "Reference": {
      // The only `Reference(...)` arg in the codegen today is
      // `Reference(Struct(name))` — used for `init_callback_vtable_*` to pass
      // a vtable struct in linear memory. The allocation is persistent: Rust
      // retains the pointer for the cdylib lifetime, so it is deliberately
      // not freed from the scratch path.
      if (t.inner.tag !== "Struct") {
        throw new Error(
          `planArg: Reference inner type ${t.inner.tag} not yet supported`,
        );
      }
      const structName = t.inner.name;
      return {
        ...argSlot(t),
        prepare: (ctx, _base, value: Record<string, unknown>) => {
          const layout = ctx.structs.get(structName);
          if (!layout) {
            throw new Error(`planArg: struct "${structName}" not registered`);
          }
          const ptr = ctx.alloc(layout.size, 8);
          for (const f of layout.fields) {
            const fieldVal = value[f.name];
            if (f.type.tag === "Callback") {
              const def = ctx.callbackDefs.get(f.type.name);
              if (!def) {
                throw new Error(
                  `planArg: callback shape "${f.type.name}" (in struct "${structName}") not registered`,
                );
              }
              const slot = ctx.installCallback(
                fieldVal as (...args: any[]) => any,
                def,
              );
              ctx.memory.writeU32(ptr + f.offset, slot);
            } else {
              f.shape.write(ctx.memory, ptr + f.offset, fieldVal);
            }
          }
          return ptr;
        },
      };
    }
    default:
      throw new Error(`planArg: type ${t.tag} not yet supported`);
  }
}

// Copy a wasm-side RustBuffer's payload into a fresh `Uint8Array` and free
// the wasm allocation. Used for RustCallStatus's errorBuf, whose bytes reach
// `liftString` / errorHandler after the allocation is gone and so must be
// JS-owned.
function copyAndFreeRustBuffer(
  ctx: DispatchContext,
  rb: RustBufferLike,
): Uint8Array {
  const len = Number(rb.len);
  const cap = Number(rb.capacity);
  const bytes =
    len > 0 ? ctx.memory.readBytes(rb.dataPtr, len) : new Uint8Array(0);
  if (cap > 0 && rb.dataPtr !== 0) {
    ctx.free(rb.dataPtr, cap, 1);
  }
  return bytes;
}

// Records a view's Rust-side `capacity`, which `rustbuffer_free` must pass
// back to satisfy the allocator's `Layout` contract.
//
// A handed-off view has `byteLength === len`, so converters see only the
// message bytes — but Rust may have allocated more. Views from elsewhere
// (`rustbuffer_alloc(n)`) carry no hint, and the runtime falls back to
// `byteLength`, which equals capacity for those.
const CAPACITY_HINT = Symbol("uniffi.rustbuffer.capacity");
function setCapacityHint(view: Uint8Array, capacity: number): void {
  (view as any)[CAPACITY_HINT] = capacity;
}
export function getCapacityHint(view: Uint8Array): number | undefined {
  return (view as any)[CAPACITY_HINT];
}

// Alias the RustBuffer's payload in wasm memory — no copy, no free. The one
// mandatory copy happens inside `lift()` instead.
//
// Two invariants the caller owns:
//
//   * Ownership. The codegen call site wraps `lift(view)` in `try/finally`
//     so `rustbuffer_free(view)` runs even when `lift` throws.
//   * Detachment. Growing wasm memory invalidates the view, so `lift` must
//     not trigger a grow before it finishes reading. Converter `read` only
//     allocates on the JS heap, so this holds today.
function viewRustBufferHandoff(
  ctx: DispatchContext,
  rb: RustBufferLike,
): Uint8Array {
  const cap = Number(rb.capacity);
  const len = Number(rb.len);
  if (cap === 0 || rb.dataPtr === 0) return new Uint8Array(0);
  const view = new Uint8Array(ctx.memory.buffer(), rb.dataPtr, len);
  if (cap !== len) setCapacityHint(view, cap);
  return view;
}

function planRet(t: FfiTypeDesc): RetPlan {
  switch (t.tag) {
    case "Void":
      return { ...retSlot(t), finish: () => undefined };
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "Handle":
    case "UInt64":
    case "Int64":
    case "Float32":
    case "Float64":
      return { ...retSlot(t), finish: (_c, _b, r) => r };
    case "RustBuffer":
      // Hand back a `Uint8Array` view aliasing wasm memory. Codegen wraps the
      // converter's `lift(view)` in try/finally so `rustbuffer_free(view)`
      // releases the allocation even when `lift` throws.
      return {
        ...retSlot(t),
        finish: (ctx, sretBase) =>
          viewRustBufferHandoff(ctx, readRustBuffer(ctx.memory, sretBase)),
      };
    default:
      throw new Error(`planRet: type ${(t as any).tag} not yet supported`);
  }
}

interface CallLayout {
  layoutSize: number;
  statusOff: number; // -1 if !hasRustCallStatus
  sretOff: number; // -1 if the return needs no sret slot
  argOffsets: number[]; // -1 entries for scalar args (no scratch slot)
  base: number; // wasm-memory offset of the reserved region
}

/**
 * Lay out one call's scratch — status, sret, then each arg needing a slot —
 * and reserve the region. Takes `ScratchSlot`s rather than plans so both
 * dispatchers can call it: the JIT path knows the sizes without building the
 * `prepare`/`finish` closures it would never call.
 */
function computeAndReserveLayout(
  ctx: DispatchContext,
  def: FunctionDef,
  argSlots: ScratchSlot[],
  ret: RetSlot,
): CallLayout {
  let layoutSize = 0;
  const statusOff = def.hasRustCallStatus
    ? ((layoutSize = align(layoutSize, 8)), layoutSize)
    : -1;
  if (def.hasRustCallStatus) layoutSize += RUST_CALL_STATUS_SIZE;
  const sretOff = ret.hasSret
    ? ((layoutSize = align(layoutSize, ret.scratchAlign)), layoutSize)
    : -1;
  if (ret.hasSret) layoutSize += ret.scratchSize;
  const argOffsets: number[] = argSlots.map((s) => {
    if (s.scratchSize === 0) return -1;
    layoutSize = align(layoutSize, s.scratchAlign);
    const off = layoutSize;
    layoutSize += s.scratchSize;
    return off;
  });
  const base = ctx.scratch.reserve(layoutSize);
  return { layoutSize, statusOff, sretOff, argOffsets, base };
}

/**
 * Interpreted dispatcher: reserve scratch once, then per call write each arg
 * via its `prepare` closure, call `exportFn`, check the status inline, and
 * return `retPlan.finish`.
 *
 * Every call shares the one register-time scratch reservation, so an export
 * that re-enters itself synchronously corrupts the outer call. See
 * `Scratch.reserve`.
 */
function buildInterpretedDispatcher(
  ctx: DispatchContext,
  exportFn: Function,
  def: FunctionDef,
): (...jsArgs: any[]) => any {
  const argPlans = def.args.map((t) => planArg(t));
  const retPlan = planRet(def.ret);
  const { statusOff, sretOff, argOffsets, base } = computeAndReserveLayout(
    ctx,
    def,
    argPlans,
    retPlan,
  );

  return function dispatch(...jsArgs: any[]) {
    // Last JS arg is the status object iff hasRustCallStatus.
    const userArgs = def.hasRustCallStatus ? jsArgs.slice(0, -1) : jsArgs;
    const statusObj = def.hasRustCallStatus ? jsArgs[jsArgs.length - 1] : null;
    // Per-call cleanup closures collected from `prepare`. The `try` starts
    // before the prepare loop so storage allocated by an earlier arg is
    // released even if a later `prepare` throws.
    const cleanups: Array<() => void> = [];

    try {
      if (def.hasRustCallStatus)
        writeRustCallStatusZero(ctx.memory, base + statusOff);

      const wasmArgs: any[] = [];
      if (retPlan.hasSret) wasmArgs.push(base + sretOff);
      for (let i = 0; i < argPlans.length; i++) {
        const p = argPlans[i];
        const argBase = argOffsets[i] >= 0 ? base + argOffsets[i] : 0;
        const prepared = p.prepare(ctx, argBase, userArgs[i]);
        // Objects are `PreparedArg`; numbers and bigints are scalar words.
        if (typeof prepared === "object" && prepared !== null) {
          wasmArgs.push(prepared.word);
          if (prepared.cleanup) cleanups.push(prepared.cleanup);
        } else {
          wasmArgs.push(prepared);
        }
      }
      if (def.hasRustCallStatus) wasmArgs.push(base + statusOff);

      const scalarRet = (exportFn as any)(...wasmArgs);

      // Inline status check on the success path — no RustCallStatus object built.
      if (def.hasRustCallStatus) {
        const code = ctx.memory.readU8(base + statusOff);
        statusObj.code = code;
        if (code !== 0) {
          // Lift errorBuf into a Uint8Array and free the wasm allocation, to
          // match the `UniffiByteArray` contract `rust-call.ts` assumes when
          // it forwards the buffer to liftString / errorHandler.
          const errBufRb = readRustBuffer(
            ctx.memory,
            base + statusOff + RCS_ERROR_BUF_OFF,
          );
          (statusObj as any).errorBuf = copyAndFreeRustBuffer(ctx, errBufRb);
          // On the error path Rust hasn't written a valid value into the sret
          // slot — `rustCallWithError` will throw via `uniffiCheckCallStatus`
          // before anything looks at the return, so skip `retPlan.finish`.
          return undefined;
        }
      }
      return retPlan.finish(ctx, base + sretOff, scalarRet);
    } finally {
      // Release transient wasm storage the callee only borrowed
      // (`ForeignBytes`), on the error path too. Each closure captures its own
      // call's allocation, so re-entrant calls never free each other's.
      for (const cleanup of cleanups) cleanup();
    }
  };
}

/**
 * Tags that the JIT dispatcher knows how to emit inline as return types.
 * Includes `Void` (return-only), scalars, and `RustBuffer`.
 */
function isSimpleRetType(t: FfiTypeDesc): boolean {
  switch (t.tag) {
    case "Void":
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "UInt64":
    case "Int64":
    case "Float32":
    case "Float64":
    case "Handle":
    case "RustBuffer":
      return true;
    default:
      return false;
  }
}

/**
 * Tags that the JIT dispatcher knows how to emit inline as arg types.
 * Restricted to scalars and `RustBuffer` — excludes `Void` (return-only).
 * `ForeignBytes` is deliberately absent: lowering it copies the bytes and
 * frees them after the call, and the JIT body has no post-call hook for that.
 * Omitting it here makes `buildJitDispatcher` return `undefined` so
 * `specializeFunction` takes the interpreted path, which does the cleanup.
 * Anything outside this set returns `undefined` from `buildJitDispatcher`,
 * causing `specializeFunction` to fall back to the interpreted path.
 */
function isSimpleArgType(t: FfiTypeDesc): boolean {
  switch (t.tag) {
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "UInt64":
    case "Int64":
    case "Float32":
    case "Float64":
    case "Handle":
    case "RustBuffer":
      return true;
    default:
      return false;
  }
}

function isI32Tag(tag: string): boolean {
  return (
    tag === "UInt8" ||
    tag === "Int8" ||
    tag === "UInt16" ||
    tag === "Int16" ||
    tag === "UInt32" ||
    tag === "Int32"
  );
}

function isBigIntTag(tag: string): boolean {
  return tag === "UInt64" || tag === "Int64" || tag === "Handle";
}

function isFloatTag(tag: string): boolean {
  return tag === "Float32" || tag === "Float64";
}

function isPassByValueArg(tag: string): boolean {
  return isI32Tag(tag) || isBigIntTag(tag) || isFloatTag(tag);
}

function safeName(name: string): string {
  return name.replace(/[^A-Za-z0-9_]/g, "_") || "dispatch";
}

function buildReturnReader(retTag: string, sretBase: number): string {
  switch (retTag) {
    case "Void":
      return "return undefined;";
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "UInt64":
    case "Int64":
    case "Handle":
    case "Float32":
    case "Float64":
      return "return scalarRet;";
    case "RustBuffer":
      // As in `planRet`: the caller's try/finally frees the returned view.
      return `return viewRustBufferHandoff(ctx, readRustBuffer(mem, ${sretBase}));`;
    default:
      // Unreachable: `isSimpleRetType` filters before this is called.
      return "throw new Error('JIT: unsupported return type');";
  }
}

/**
 * Build a JIT-compiled dispatcher for a function whose arg/return types are
 * all in the "simple" set (scalars + `RustBuffer` + `Void` return). Returns
 * `undefined` for unsupported shapes (callback args, struct-by-value args,
 * etc.) so the caller falls back to the interpreted dispatcher.
 *
 * Offsets, scalar/RustBuffer call shape, and the inline status check are
 * baked into the body. The body refers to `mem`, `fn`, `alloc`, `ctx` and
 * a handful of helpers as free variables; they are passed in as
 * `new Function` constructor args so V8 can monomorphise the closure
 * identically to hand-written source.
 *
 * The body bakes in one register-time scratch base, so it carries the same
 * re-entrancy hazard as `buildInterpretedDispatcher`. See `Scratch.reserve`.
 */
export function buildJitDispatcher(
  ctx: DispatchContext,
  exportFn: Function,
  def: FunctionDef,
  fnName: string,
): ((...args: any[]) => unknown) | undefined {
  // Bail out for unsupported shapes — fall back to interpreted.
  for (const a of def.args) {
    if (!isSimpleArgType(a)) return undefined;
  }
  if (!isSimpleRetType(def.ret)) return undefined;

  const { statusOff, sretOff, argOffsets, base } = computeAndReserveLayout(
    ctx,
    def,
    def.args.map(argSlot),
    retSlot(def.ret),
  );

  // String-build the dispatcher body.
  const hasStatus = def.hasRustCallStatus;
  const userArgNames = def.args.map((_, i) => `a${i}`);
  const paramNames = hasStatus
    ? [...userArgNames, "statusObj"]
    : [...userArgNames];

  // Per-arg: scalars pass straight through; RustBuffer writes a 24-byte
  // struct into the reserved scratch slot.
  const writes: string[] = [];
  for (let i = 0; i < def.args.length; i++) {
    const tag = def.args[i].tag;
    if (isPassByValueArg(tag)) {
      // Nothing to write — value goes straight into the wasm call below.
    } else if (tag === "RustBuffer") {
      const off = base + argOffsets[i];
      writes.push(`writeRustBufferPayload(mem, alloc, ${off}, a${i});`);
    } else {
      // isSimpleArgType filtered above; this is unreachable.
      throw new Error(`unreachable: arg tag ${tag} passed isSimpleArgType`);
    }
  }

  // Wasm-side arg order: [sret?, ...userArgs, status?].
  const callArgs: string[] = [];
  if (sretOff >= 0) callArgs.push(`${base + sretOff}`);
  for (let i = 0; i < def.args.length; i++) {
    const tag = def.args[i].tag;
    if (isI32Tag(tag)) {
      // Match the interpreted path's `v | 0` coercion.
      callArgs.push(`(a${i} | 0)`);
    } else if (isPassByValueArg(tag)) {
      // bigint / float pass straight through.
      callArgs.push(`a${i}`);
    } else {
      // RustBuffer: the wasm fn receives the scratch pointer.
      callArgs.push(`${base + argOffsets[i]}`);
    }
  }
  if (hasStatus) callArgs.push(`${base + statusOff}`);

  const statusBase = base + statusOff;
  const errBufBase = statusBase + RCS_ERROR_BUF_OFF;
  const writeStatusZero = hasStatus
    ? `writeRustCallStatusZero(mem, ${statusBase});`
    : "";
  // Mirrors `buildInterpretedDispatcher`: assign `statusObj.code` on both
  // paths, and on error copy out errorBuf then return undefined — Rust
  // leaves the sret slot invalid when the call fails.
  const statusCheck = hasStatus
    ? `
      var code = mem.readU8(${statusBase});
      statusObj.code = code;
      if (code !== 0) {
        statusObj.errorBuf = copyAndFreeRustBuffer(ctx, readRustBuffer(mem, ${errBufBase}));
        return undefined;
      }
    `
    : "";
  const callExpr = `fn(${callArgs.join(", ")})`;
  const readRet = buildReturnReader(def.ret.tag, base + sretOff);

  const body = `
    "use strict";
    return function ${safeName(fnName)}(${paramNames.join(", ")}) {
      ${writeStatusZero}
      ${writes.join("\n      ")}
      var scalarRet = ${callExpr};
      ${statusCheck}
      ${readRet}
    };
  `;

  // eslint-disable-next-line no-new-func
  const factory = new Function(
    "ctx",
    "mem",
    "fn",
    "alloc",
    "readRustBuffer",
    "viewRustBufferHandoff",
    "copyAndFreeRustBuffer",
    "writeRustBufferPayload",
    "writeRustCallStatusZero",
    body,
  );
  return factory(
    ctx,
    ctx.memory,
    exportFn,
    ctx.alloc,
    readRustBuffer,
    viewRustBufferHandoff,
    copyAndFreeRustBuffer,
    writeRustBufferPayload,
    writeRustCallStatusZero,
  ) as (...args: any[]) => unknown;
}

export function specializeFunction(
  ctx: DispatchContext,
  exportFn: Function,
  def: FunctionDef,
  fnName: string = "dispatch",
): (...jsArgs: any[]) => any {
  if (ctx.useJit) {
    const jit = buildJitDispatcher(ctx, exportFn, def, fnName);
    if (jit) return jit as (...jsArgs: any[]) => any;
  }
  return buildInterpretedDispatcher(ctx, exportFn, def);
}
