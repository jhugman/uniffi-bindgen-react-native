/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// JS-side dispatch table and per-shape trampoline cache for the Wasm2 player.
//
// Rust can only call back into JS through `call_indirect`, so every callback
// needs a wasm function in the host's `__indirect_function_table`. Three
// pieces make that work:
//
//   * A *shape* is one signature, say "(i32, i32) -> i32". Each gets one
//     trampoline, emitted by `trampoline.ts` and instantiated as its own
//     module: it imports `env.__ubrn_dispatch` and exports "trampoline".
//   * Many closures share their shape's trampoline. `register` hands each a
//     *handle*, unique within the shape, which the host passes as the first
//     argument.
//   * `dispatch(shapeId, handle, ...args)` recovers the closure and calls it.

import type { FfiTypeDesc } from "./ffi-type.js";
import type { Memory } from "./memory.js";
import {
  RUST_BUFFER_SIZE,
  readRustBuffer,
  writeRustBufferPayload,
  writeRustCallStatusZero,
  type StructLayout,
} from "./marshal.js";
import { emitTrampoline as emitTrampolineModule } from "./trampoline.js";

const RCS_CODE_OFF = 0;
const RCS_ERROR_BUF_OFF = 8;

/** Lift one callback arg into the form the codegen closure expects: the
 * receive-side mirror of `planArg` in `call.ts`.
 *
 * Scalars pass through. A `RustBuffer` becomes a `Uint8Array` and its wasm
 * allocation is freed, Rust having handed ownership over. A `Callback` is a
 * table index, wrapped into a JS callable. */
function liftCallbackArg(ctx: LiftContext, t: FfiTypeDesc, raw: any): any {
  if (t.tag === "RustBuffer") {
    const ptr = raw as number;
    const rb = readRustBuffer(ctx.memory, ptr);
    const len = Number(rb.len);
    const cap = Number(rb.capacity);
    const bytes =
      len > 0 ? ctx.memory.readBytes(rb.dataPtr, len) : new Uint8Array(0);
    if (cap > 0 && rb.dataPtr !== 0) {
      ctx.free(rb.dataPtr, cap, 1);
    }
    return bytes;
  }
  if (t.tag === "Callback") {
    return wrapWasmCallback(ctx, t.name, raw as number);
  }
  return raw;
}

interface LiftContext {
  memory: Memory;
  alloc: (size: number, align: number) => number;
  free: (ptr: number, size: number, align: number) => void;
  table?: WebAssembly.Table;
  callbackDefs?: Map<string, CallbackDef>;
  structs?: Map<string, StructLayout>;
}

/** Wrap a function-table index into a JS callable that lowers its arguments
 * per `def.args` and dispatches through `__indirect_function_table`, the way
 * `call_indirect` would. */
function wrapWasmCallback(
  ctx: LiftContext,
  name: string,
  slotIdx: number,
): (...args: any[]) => any {
  const callbackDefs = ctx.callbackDefs;
  const structs = ctx.structs;
  const table = ctx.table;
  if (!callbackDefs || !structs || !table) {
    throw new Error(
      "wrapWasmCallback: registration context not installed; call setRegistrationContext()",
    );
  }
  const def = callbackDefs.get(name);
  if (!def) {
    throw new Error(
      `wrapWasmCallback: callback shape "${name}" not registered`,
    );
  }
  const target = table.get(slotIdx);
  if (typeof target !== "function") {
    throw new Error(
      `wrapWasmCallback: __indirect_function_table[${slotIdx}] is not a function`,
    );
  }
  // No completion callback lifted this way sets either flag. Bail rather
  // than silently produce wrong wasm args if one ever does.
  if (def.outReturn || def.hasRustCallStatus) {
    throw new Error(
      `wrapWasmCallback: callback shape "${name}" with outReturn/hasRustCallStatus not yet supported`,
    );
  }
  const argTypes = def.args;
  const memory = ctx.memory;
  const alloc = ctx.alloc;
  const free = ctx.free;
  return (...userArgs: any[]) => {
    // Freed after the call. Inner allocations, such as RustBuffer payloads,
    // pass to Rust and are not.
    const outerAllocs: { ptr: number; size: number; align: number }[] = [];
    const wasmArgs: any[] = [];
    try {
      for (let i = 0; i < argTypes.length; i++) {
        const t = argTypes[i];
        const v = userArgs[i];
        wasmArgs.push(
          lowerCallbackArg(memory, alloc, structs, t, v, outerAllocs),
        );
      }
      return (target as (...a: any[]) => any)(...wasmArgs);
    } finally {
      for (const a of outerAllocs) free(a.ptr, a.size, a.align);
    }
  };
}

function lowerCallbackArg(
  memory: Memory,
  alloc: (size: number, align: number) => number,
  structs: Map<string, StructLayout>,
  t: FfiTypeDesc,
  v: any,
  outerAllocs: { ptr: number; size: number; align: number }[],
): any {
  switch (t.tag) {
    case "UInt8":
    case "Int8":
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "VoidPointer":
    case "Callback":
    case "Reference":
    case "MutReference":
      return v | 0;
    case "UInt64":
    case "Int64":
    case "Handle":
      return typeof v === "bigint" ? v : BigInt(v);
    case "Float32":
    case "Float64":
      return v;
    case "RustBuffer": {
      // Bytes are handed to Rust, so don't free them here.
      const rbPtr = alloc(RUST_BUFFER_SIZE, 8);
      outerAllocs.push({ ptr: rbPtr, size: RUST_BUFFER_SIZE, align: 8 });
      writeRustBufferPayload(memory, alloc, rbPtr, v as Uint8Array);
      return rbPtr;
    }
    case "Struct": {
      const layout = structs.get(t.name);
      if (!layout) {
        throw new Error(`lowerCallbackArg: struct "${t.name}" not registered`);
      }
      const ptr = alloc(layout.size, 8);
      outerAllocs.push({ ptr, size: layout.size, align: 8 });
      for (const f of layout.fields) {
        const fieldVal = (v as any)[f.name];
        switch (f.type.tag) {
          case "RustCallStatus":
            writeStatusFromResult(memory, alloc, ptr + f.offset, fieldVal);
            break;
          case "RustBuffer":
            writeRustBufferPayload(
              memory,
              alloc,
              ptr + f.offset,
              fieldVal as Uint8Array,
            );
            break;
          default:
            f.shape.write(memory, ptr + f.offset, fieldVal);
        }
      }
      return ptr;
    }
    default:
      throw new Error(`lowerCallbackArg: type ${t.tag} not yet supported`);
  }
}

/** Write a lowered return value to the pointer Rust handed us: `planRet` in
 * `call.ts`, in the lowering direction. */
function writeReturnValue(
  memory: Memory,
  alloc: (size: number, align: number) => number,
  ptr: number,
  ret: FfiTypeDesc,
  value: any,
  /** Optional registration context, required when `ret.tag === "Struct"`
   * (or when a struct field is `Callback`-typed). The dispatcher passes
   * this through from `makeDispatchRoute`. */
  ctx?: {
    structs: Map<string, StructLayout>;
    callbackDefs: Map<string, CallbackDef>;
    installCallback: (fn: (...args: any[]) => any, def: CallbackDef) => number;
  },
): void {
  switch (ret.tag) {
    case "Void":
      return;
    case "UInt8":
    case "Int8":
      memory.writeU8(ptr, value | 0);
      return;
    case "UInt16":
    case "Int16":
    case "UInt32":
    case "Int32":
    case "VoidPointer":
    case "Callback":
    case "Reference":
    case "MutReference":
      memory.writeU32(ptr, value | 0);
      return;
    case "UInt64":
    case "Int64":
    case "Handle":
      memory.writeU64(ptr, typeof value === "bigint" ? value : BigInt(value));
      return;
    case "Float32":
      memory.writeF32(ptr, value);
      return;
    case "Float64":
      memory.writeF64(ptr, value);
      return;
    case "RustBuffer":
      // `value` is a `Uint8Array` (UniffiByteArray) — round-trip through
      // wasm memory the same way `planArg.RustBuffer` does on the lower
      // path, so Rust can reclaim it via `Vec::from_raw_parts`.
      writeRustBufferPayload(memory, alloc, ptr, value as Uint8Array);
      return;
    case "Struct": {
      if (!ctx) {
        throw new Error(
          `writeReturnValue: Struct return ("${ret.name}") requires a registration context`,
        );
      }
      const layout = ctx.structs.get(ret.name);
      if (!layout) {
        throw new Error(
          `writeReturnValue: struct "${ret.name}" not registered`,
        );
      }
      // Walk the struct fields in declaration order, writing each at its
      // computed offset. `Callback`-typed fields need the JS function
      // installed as a wasm callback (so Rust receives a function-table
      // index it can `call_indirect` on). `RustBuffer` fields ship their
      // payload bytes through wasm memory and transfer ownership to Rust.
      // Other field types are handled by the precomputed `f.shape.write`.
      for (const f of layout.fields) {
        const fieldVal = (value as any)[f.name];
        switch (f.type.tag) {
          case "Callback": {
            const def = ctx.callbackDefs.get(f.type.name);
            if (!def) {
              throw new Error(
                `writeReturnValue: callback shape "${f.type.name}" not registered (in struct "${ret.name}")`,
              );
            }
            const slot = ctx.installCallback(
              fieldVal as (...args: any[]) => any,
              def,
            );
            memory.writeU32(ptr + f.offset, slot);
            break;
          }
          case "RustBuffer":
            writeRustBufferPayload(
              memory,
              alloc,
              ptr + f.offset,
              fieldVal as Uint8Array,
            );
            break;
          default:
            f.shape.write(memory, ptr + f.offset, fieldVal);
        }
      }
      return;
    }
    default:
      throw new Error(`writeReturnValue: ret type ${ret.tag} not supported`);
  }
}

/** Write a `UniffiRustCallStatus` (32 bytes: i8 code + RustBuffer) to wasm
 * memory. Empty error → success path. */
function writeStatusFromResult(
  memory: Memory,
  alloc: (size: number, align: number) => number,
  ptr: number,
  result: any,
): void {
  const code = (result?.code ?? 0) | 0;
  if (code === 0) {
    writeRustCallStatusZero(memory, ptr);
    return;
  }
  memory.writeU8(ptr + RCS_CODE_OFF, code);
  const errorBuf = (result?.errorBuf as Uint8Array | undefined) ?? EMPTY_BYTES;
  writeRustBufferPayload(memory, alloc, ptr + RCS_ERROR_BUF_OFF, errorBuf);
}

const EMPTY_BYTES = new Uint8Array(0);

const TAG_I32 = 0;
const TAG_I64 = 1;
const TAG_F32 = 2;
const TAG_F64 = 3;

function tagFor(t: FfiTypeDesc): number {
  switch (t.tag) {
    case "UInt64":
    case "Int64":
    // UniFFI handles are u64 on every platform, including wasm32. Lowering
    // them as i32 here would emit a trampoline whose `call_indirect`
    // signature didn't match Rust's `extern "C" fn(handle: u64, ...)` —
    // the wasm runtime then throws "null function or function signature
    // mismatch" at the first poll.
    case "Handle":
      return TAG_I64;
    case "Float32":
      return TAG_F32;
    case "Float64":
      return TAG_F64;
    default:
      // All pointer-sized and small-integer FFI types lower to i32 in the
      // wasm ABI. (Void is invalid here — only used as a return type and
      // handled separately by `signatureKey`/`buildDescriptor`.)
      return TAG_I32;
  }
}

export interface CallbackDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
  outReturn?: boolean;
}

interface ShapeCache {
  signature: string;
  shapeId: number;
  trampoline: WebAssembly.ExportValue;
  registry: Map<number, (...args: any[]) => any>;
  nextHandle: number;
}

/**
 * Per-instance dispatch table. `module.ts` owns one of these and routes the
 * `__ubrn_dispatch` import through `dispatch`.
 */
export class CallbackTable {
  private shapes = new Map<string, ShapeCache>();
  private shapesByShapeId: ShapeCache[] = [];
  /**
   * Shapes that bypass the registry-by-handle path. The `RustFutureContinu-
   * ationCallback` shape lives here: its first argument is a u64 cookie, not
   * a UniFFI registry handle, so the per-shape callback owns its own routing
   * (e.g. the FutureRegistry's cookie -> resolver map).
   */
  private continuationShapes = new Map<number, (...args: any[]) => any>();

  /** Optional registration context: callback shape defs + compiled struct
   * layouts. Set by `module.ts` after `registerSync()` so `makeDispatchRoute`
   * can resolve `Callback`-typed args (function-table indices) into JS
   * callables that re-lower their args and dispatch through the indirect
   * function table. */
  private registrationContext?: {
    callbackDefs: Map<string, CallbackDef>;
    structs: Map<string, StructLayout>;
  };

  constructor(
    private memory: Memory,
    private hostExports: WebAssembly.Exports,
    /** Async compilation is not implemented; `false` is reserved for it. */
    private compileSync: boolean = true,
  ) {}

  setRegistrationContext(ctx: {
    callbackDefs: Map<string, CallbackDef>;
    structs: Map<string, StructLayout>;
  }): void {
    this.registrationContext = ctx;
  }

  /**
   * Implements `env.__ubrn_dispatch`. Trampolines prepend their shape id and
   * forward everything else verbatim.
   *
   * On the registry path the host's first argument is the handle, per the
   * UniFFI vtable convention; it is stripped before calling the closure.
   *
   * For shapes registered via `registerContinuationShape` (e.g. the future
   * continuation callback) we forward all args verbatim — `args[0]` is a
   * cookie, not a handle, and the caller owns the lookup.
   */
  dispatch = (shapeId: number, ...args: any[]): any => {
    const continuationFn = this.continuationShapes.get(shapeId);
    if (continuationFn) {
      return continuationFn(...args);
    }
    const cache = this.shapesByShapeId[shapeId];
    if (!cache) {
      throw new Error(`__ubrn_dispatch: unknown shape ${shapeId}`);
    }
    // The trampoline forwards the handle in whatever wasm type the shape
    // declares — a `Handle`/u64 arrives as a bigint, everything else as a
    // number. `registry` is keyed by the number `register()` handed out, so
    // normalise before looking up.
    const handle = Number(args[0]);
    const cb = cache.registry.get(handle);
    if (!cb) {
      throw new Error(
        `__ubrn_dispatch: unknown handle ${handle} for shape ${shapeId}`,
      );
    }
    return cb(...args.slice(1));
  };

  /**
   * Mark `shapeId` as a "continuation" shape — when the trampoline fires for
   * that shape we route directly to `fn(...args)` rather than going through
   * the registry-by-handle lookup. Used by `FutureRegistry`.
   */
  registerContinuationShape(
    shapeId: number,
    fn: (...args: any[]) => any,
  ): void {
    this.continuationShapes.set(shapeId, fn);
  }

  // Slots for closures installed directly rather than registered against a
  // shape: `planArg`'s `Callback(name)` args and `Callback`-typed vtable
  // fields. Each distinct (closure, signature) pair takes its own shape and
  // table slot. That stays bounded because codegen builds each callback
  // interface's vtable once at module scope and carries instance identity in
  // the handle argument, so an interface costs N + 2 (clone, free) shapes
  // however many instances exist.
  //
  // Keyed by signature as well as identity: one function installed for two
  // signatures needs two slots, or Rust's `call_indirect` traps.
  private fnSlotCache = new WeakMap<Function, Map<string, number>>();

  installCallbackFunction(
    fn: (...args: any[]) => any,
    def: CallbackDef,
  ): number {
    const sigKey = this.signatureKey(def);
    let bySig = this.fnSlotCache.get(fn);
    const cached = bySig?.get(sigKey);
    if (cached !== undefined) return cached;

    const shapeId = this.shapesByShapeId.length;
    const wasmBytes = this.compileTrampoline(shapeId, def);
    const trampolineModule = new WebAssembly.Module(wasmBytes as BufferSource);
    const instance = new WebAssembly.Instance(trampolineModule, {
      env: { __ubrn_dispatch: this.dispatch },
    });
    const trampoline = instance.exports.trampoline;
    if (typeof trampoline !== "function") {
      throw new Error(
        "emitted trampoline module did not export a 'trampoline' function",
      );
    }
    const cache: ShapeCache = {
      signature: sigKey,
      shapeId,
      trampoline,
      registry: new Map(),
      nextHandle: 1,
    };
    this.shapesByShapeId.push(cache);

    // Vtable methods on the codegen side return a `UniffiResult`; their
    // wasm-level signature however returns void and takes trailing
    // `out_ret` / `out_status` pointer args. Wrap the user closure so the
    // dispatcher unwraps the result and writes it back through those
    // pointers. Free/clone (no `outReturn`/`hasRustCallStatus`) and the
    // future continuation route through unchanged.
    const route = this.makeDispatchRoute(fn, def);
    // Routed through the continuation path so all wasm args (including the
    // u64 instance handle in vtable methods) are forwarded verbatim — the JS
    // closure does its own handle-to-instance lift.
    this.continuationShapes.set(shapeId, route);

    const table = this.hostExports.__indirect_function_table as
      | WebAssembly.Table
      | undefined;
    if (!table) {
      throw new Error(
        "installCallbackFunction: host wasm does not export __indirect_function_table",
      );
    }
    const slot = table.length;
    table.grow(1);
    table.set(slot, trampoline as any);

    if (!bySig) {
      bySig = new Map();
      this.fnSlotCache.set(fn, bySig);
    }
    bySig.set(sigKey, slot);
    return slot;
  }

  private makeDispatchRoute(
    fn: (...args: any[]) => any,
    def: CallbackDef,
  ): (...args: any[]) => any {
    const userArgCount = def.args.length;
    const memory = this.memory;
    const alloc = this.hostExports.__ubrn_alloc as (
      size: number,
      align: number,
    ) => number;
    const free = this.hostExports.__ubrn_free as (
      ptr: number,
      size: number,
      align: number,
    ) => void;
    const argTypes = def.args;
    // Aggregate-typed args (RustBuffer) and function-pointer args
    // (`Callback`) need a wasm-side lift before being handed to the user
    // closure: the wasm ABI passes those as i32 pointers / table indices,
    // but the codegen closure expects the lifted JS representation
    // (`Uint8Array` / a callable wrapper).
    const needsArgLift = argTypes.some(
      (t) => t.tag === "RustBuffer" || t.tag === "Callback",
    );
    const ret = def.ret;
    const outReturn = def.outReturn;
    const hasStatus = def.hasRustCallStatus;
    if (!outReturn && !hasStatus && !needsArgLift) {
      // Direct passthrough — free/clone, the future continuation, etc.
      return fn;
    }
    const installCallback = (
      cbFn: (...args: any[]) => any,
      cbDef: CallbackDef,
    ) => this.installCallbackFunction(cbFn, cbDef);
    const writeRetCtx = this.registrationContext
      ? {
          structs: this.registrationContext.structs,
          callbackDefs: this.registrationContext.callbackDefs,
          installCallback,
        }
      : undefined;
    const liftCtx: LiftContext = {
      memory,
      alloc,
      free,
      table: this.hostExports.__indirect_function_table as
        | WebAssembly.Table
        | undefined,
      callbackDefs: this.registrationContext?.callbackDefs,
      structs: this.registrationContext?.structs,
    };
    // `hasStatus` also decides where the FFI return value lives: sync vtable
    // methods wrap it in a `UniffiResult` (`result.pointee`), async ones
    // return it directly.
    return (...wasmArgs: any[]) => {
      let idx = userArgCount;
      const outRetPtr = outReturn ? wasmArgs[idx++] : -1;
      const outStatusPtr = hasStatus ? wasmArgs[idx++] : -1;
      const userArgs = needsArgLift
        ? argTypes.map((t, i) => liftCallbackArg(liftCtx, t, wasmArgs[i]))
        : wasmArgs.slice(0, userArgCount);
      const result = fn(...userArgs);
      const code = hasStatus ? (result?.code ?? 0) | 0 : 0;
      if (outRetPtr >= 0 && code === 0) {
        const retValue = hasStatus ? result?.pointee : result;
        writeReturnValue(memory, alloc, outRetPtr, ret, retValue, writeRetCtx);
      }
      if (outStatusPtr >= 0) {
        writeStatusFromResult(memory, alloc, outStatusPtr, result);
      }
    };
  }

  /**
   * Define (or look up, if already defined) the trampoline for a callback
   * shape. Returns the shape id, which is the index callers should pass to
   * `register`/`installAt`.
   */
  defineShape(_name: string, def: CallbackDef): number {
    const key = this.signatureKey(def);
    const existing = this.shapes.get(key);
    if (existing) {
      return existing.shapeId;
    }
    const shapeId = this.shapesByShapeId.length;

    const wasmBytes = this.compileTrampoline(shapeId, def);

    if (!this.compileSync) {
      throw new Error(
        "Async trampoline compilation not yet supported; pass compileSync=true",
      );
    }
    const trampolineModule = new WebAssembly.Module(wasmBytes as BufferSource);
    const instance = new WebAssembly.Instance(trampolineModule, {
      env: { __ubrn_dispatch: this.dispatch },
    });
    const trampoline = instance.exports.trampoline;
    if (typeof trampoline !== "function") {
      throw new Error(
        "emitted trampoline module did not export a 'trampoline' function",
      );
    }

    const cache: ShapeCache = {
      signature: key,
      shapeId,
      trampoline,
      registry: new Map(),
      // Handles start at 1 so a caller can reliably distinguish "no callback"
      // (0) from "registered". The host's `call_indirect` will use whatever
      // handle the dispatcher gives it.
      nextHandle: 1,
    };
    this.shapes.set(key, cache);
    this.shapesByShapeId.push(cache);
    return shapeId;
  }

  /** Register a JS closure for `def`'s shape. Returns the handle. */
  register(
    name: string,
    fn: (...args: any[]) => any,
    def: CallbackDef,
  ): number {
    const shapeId = this.defineShape(name, def);
    const cache = this.shapesByShapeId[shapeId];
    const handle = cache.nextHandle++;
    cache.registry.set(handle, fn);
    return handle;
  }

  /** Drop a previously registered callback. */
  unregister(handle: number, def: CallbackDef): void {
    const shapeId = this.shapeIdFor(def);
    const cache = this.shapesByShapeId[shapeId];
    cache.registry.delete(handle);
  }

  /** Number of live registrations for `def`'s shape. Used by tests and by
   * the generated `free` vtable hook to verify callbacks are released. */
  size(def: CallbackDef): number {
    const shapeId = this.shapeIdFor(def);
    return this.shapesByShapeId[shapeId].registry.size;
  }

  /**
   * Install a shape's trampoline into the host's indirect function table at
   * `tableIndex`. The host wasm calls into the trampoline via
   * `call_indirect`, so this is how a JS callback becomes reachable from
   * Rust.
   *
   * Note: many callbacks of the same *shape* share one trampoline, but each
   * needs its own table slot because the host passes a fixed table index per
   * Rust-side callback object.
   */
  installAt(tableIndex: number, shapeId: number): void {
    const cache = this.shapesByShapeId[shapeId];
    if (!cache) {
      throw new Error(`installAt: shape ${shapeId} not defined`);
    }
    const table = this.hostExports.__indirect_function_table as
      | WebAssembly.Table
      | undefined;
    if (!table) {
      throw new Error(
        "installAt: host wasm does not export __indirect_function_table",
      );
    }
    if (tableIndex >= table.length) {
      table.grow(tableIndex - table.length + 1);
    }
    table.set(tableIndex, cache.trampoline as any);
  }

  private shapeIdFor(def: CallbackDef): number {
    const key = this.signatureKey(def);
    const cache = this.shapes.get(key);
    if (!cache) {
      throw new Error("shape not defined; call defineShape first");
    }
    return cache.shapeId;
  }

  private signatureKey(def: CallbackDef): string {
    const tags = def.args.map((a) => tagFor(a));
    const ret = def.ret.tag === "Void" ? "" : `:${tagFor(def.ret)}`;
    return `${tags.join(",")}${ret}`;
  }

  /** Encode the trampoline's wasm-level signature and emit the wasm bytes
   * for it via the JS-side encoder in `trampoline.ts`. The wasm-level
   * params are `def.args` plus implicit i32 pointer args UniFFI tacks on
   * when `outReturn` / `hasRustCallStatus` are set; for `outReturn` the
   * return travels via the pointer, so the trampoline's wasm return is
   * void regardless of `def.ret`. */
  private compileTrampoline(shapeId: number, def: CallbackDef): Uint8Array {
    const paramTags = def.args.map((a) => tagFor(a));
    if (def.outReturn) paramTags.push(TAG_I32); // out_ret pointer
    if (def.hasRustCallStatus) paramTags.push(TAG_I32); // out_status pointer
    const returnTag =
      !def.outReturn && def.ret.tag !== "Void" ? tagFor(def.ret) : undefined;
    return emitTrampolineModule(shapeId, paramTags, returnTag);
  }
}
