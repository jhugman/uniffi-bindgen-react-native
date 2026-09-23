/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { DispatchContext, FunctionDef } from "./call.js";
import type { FfiTypeDesc } from "./ffi-type.js";
import { readRustBuffer, writeRustBuffer } from "./marshal.js";
import { emitJspiThunk, type WasmScalar } from "./jspi-thunk.js";

const copiedResults = new WeakSet<Uint8Array>();
export const isJspiResult = (value: Uint8Array) => copiedResults.has(value);

export function ownJspiResult(
  view: Uint8Array,
  free: (view: Uint8Array) => void,
): Uint8Array {
  const copy = view.slice();
  free(view);
  copiedResults.add(copy);
  return copy;
}

function scalar(t: FfiTypeDesc): WasmScalar {
  switch (t.tag) {
    case "Int8":
    case "UInt8":
    case "Int16":
    case "UInt16":
    case "Int32":
    case "UInt32":
    case "RustBuffer":
      return "i32";
    case "Callback":
      if (t.name === "RustFutureContinuationCallback") return "i32";
      throw new Error("JSPI only supports synchronous future continuations");
    case "Int64":
    case "UInt64":
    case "Handle":
      return "i64";
    case "Float32":
      return "f32";
    case "Float64":
      return "f64";
    default:
      throw new Error(`JSPI wasm2 does not support ${t.tag} in this call`);
  }
}

/** One controller per WASM instance. No JS wrappers below the promising entry. */
export class JspiCalls {
  private modules = new Map<string, WebAssembly.Module>();
  private calls = new Map<Function, { key: string; slot: number }>();
  private failure: Error | undefined;
  private enter: (slot: number, frame: number) => Promise<void>;
  private table: WebAssembly.Table;

  constructor(private exports: WebAssembly.Exports) {
    const api = WebAssembly as typeof WebAssembly & {
      promising?: (
        fn: Function,
      ) => (slot: number, frame: number) => Promise<void>;
      Suspending?: unknown;
    };
    if (!api.promising || !api.Suspending)
      throw new Error(
        "WebAssembly JSPI is required; Node 24 needs --experimental-wasm-jspi --experimental-wasm-exnref",
      );
    const entry = exports.__ubrn_jspi_enter;
    if (typeof entry !== "function")
      throw new Error(
        "JSPI entry missing: invoke uniffi_runtime_wasm::export_jspi_entry!() in the cdylib and build with wasm-bindgen 0.2.128 or compatible JSPI support",
      );
    this.table = exports.__indirect_function_table as WebAssembly.Table;
    if (!(this.table instanceof WebAssembly.Table))
      throw new Error("JSPI requires the staged growable function table");
    this.enter = api.promising(entry);
  }

  check(): void {
    if (this.failure) throw this.failure;
  }

  private fatal(cause: unknown): Error {
    this.failure ??= new Error(
      "JSPI call failed unexpectedly; discard this WASM instance. Rust ownership is unknown, so suspended frames are retained.",
      { cause },
    );
    this.failure.name = "JspiCallError";
    return this.failure;
  }

  build(
    ctx: DispatchContext,
    target: Function,
    def: FunctionDef,
  ): (...args: any[]) => Promise<any> {
    const sret = def.ret.tag === "RustBuffer";
    const types: WasmScalar[] = [
      ...(sret ? ["i32" as const] : []),
      ...def.args.map(scalar),
      ...(def.hasRustCallStatus ? ["i32" as const] : []),
    ];
    const ret = sret || def.ret.tag === "Void" ? undefined : scalar(def.ret);
    const key = `${types.join(",")}:${ret ?? "void"}`;
    let cached = this.calls.get(target);
    if (cached && cached.key !== key)
      throw new Error("Conflicting JSPI signatures for the same WASM export");
    if (!cached) {
      let module = this.modules.get(key);
      if (!module) {
        module = new WebAssembly.Module(
          emitJspiThunk(types, ret) as BufferSource,
        );
        this.modules.set(key, module);
      }
      const instance = new WebAssembly.Instance(module, {
        env: {
          target: target as WebAssembly.ImportValue,
          memory: this.exports.memory,
        },
      });
      const slot = this.table.grow(1);
      this.table.set(slot, instance.exports.call);
      cached = { slot, key };
      this.calls.set(target, cached);
    }
    const slot = cached.slot;
    const resultOff = types.length * 8;
    const statusOff = resultOff + 8;
    const sretOff = statusOff + 32;
    let size = sretOff + (sret ? 24 : 0);
    const buffers = def.args.map((t) => {
      if (t.tag !== "RustBuffer") return -1;
      const off = size;
      size += 24;
      return off;
    });
    const consume = (ptr: number) => {
      const rb = readRustBuffer(ctx.memory, ptr);
      const copy = ctx.memory.readBytes(rb.dataPtr, Number(rb.len));
      if (rb.capacity > 0n) ctx.free(rb.dataPtr, Number(rb.capacity), 1);
      return copy;
    };
    return async (...args: any[]) => {
      this.check();
      // Capture/copy inputs before allocating the frame (allocation can grow
      // memory). Generated selected calls lower into JS-owned buffers.
      const inputs = def.args.map((t, i) => {
        if (t.tag === "Callback") {
          const def = ctx.callbackDefs.get(t.name);
          if (
            !def ||
            def.hasRustCallStatus ||
            def.outReturn ||
            def.ret.tag !== "Void" ||
            def.args.length !== 2 ||
            def.args[0].tag !== "Handle" ||
            def.args[1].tag !== "Int8"
          )
            throw new Error("Invalid JSPI future continuation definition");
          return ctx.installCallback(args[i], def);
        }
        if (t.tag !== "RustBuffer") return args[i];
        const v = args[i] as Uint8Array;
        if (!(v instanceof Uint8Array))
          throw new Error("JSPI input is not a byte buffer");
        if (v.buffer === ctx.memory.buffer())
          throw new Error(
            "JSPI calls require JS-owned inputs; use rustbuffer_alloc_jspi when lowering",
          );
        return v.slice();
      });
      const frame = ctx.alloc(size, 8);
      if (!frame) throw new Error("JSPI frame allocation failed");
      const allocated: { ptr: number; len: number }[] = [];
      let entered = false;
      let settled = false;
      try {
        ctx.memory.view().fill(0, frame, frame + size);
        const values: (number | bigint)[] = sret ? [frame + sretOff] : [];
        def.args.forEach((t, i) => {
          if (t.tag !== "RustBuffer") {
            values.push(inputs[i]);
            return;
          }
          const bytes = inputs[i] as Uint8Array;
          const ptr = bytes.length ? ctx.alloc(bytes.length, 1) : 0;
          if (bytes.length && !ptr)
            throw new Error("JSPI input allocation failed");
          if (bytes.length) {
            allocated.push({ ptr, len: bytes.length });
            ctx.memory.view().set(bytes, ptr);
          }
          const descriptor = frame + buffers[i];
          writeRustBuffer(ctx.memory, descriptor, {
            capacity: BigInt(bytes.length),
            len: BigInt(bytes.length),
            dataPtr: ptr,
          });
          values.push(descriptor);
        });
        if (def.hasRustCallStatus) values.push(frame + statusOff);
        values.forEach((v, i) => {
          const dv = ctx.memory.dv();
          const ptr = frame + i * 8;
          switch (types[i]) {
            case "i32":
              dv.setInt32(ptr, Number(v), true);
              break;
            case "i64":
              dv.setBigInt64(ptr, BigInt(v), true);
              break;
            case "f32":
              dv.setFloat32(ptr, Number(v), true);
              break;
            case "f64":
              dv.setFloat64(ptr, Number(v), true);
              break;
          }
        });
        entered = true;
        try {
          await this.enter(slot, frame);
        } catch (cause) {
          throw this.fatal(cause);
        }
        this.check();
        settled = true;
        if (def.hasRustCallStatus) {
          const status = args[def.args.length];
          status.code = ctx.memory.readU8(frame + statusOff);
          if (status.code !== 0) {
            status.errorBuf = consume(frame + statusOff + 8);
            return undefined;
          }
        }
        if (sret) {
          const result = consume(frame + sretOff);
          copiedResults.add(result);
          return result;
        }
        const dv = ctx.memory.dv();
        const ptr = frame + resultOff;
        switch (ret) {
          case "i32":
            return def.ret.tag.startsWith("UInt")
              ? dv.getUint32(ptr, true)
              : dv.getInt32(ptr, true);
          case "i64":
            return def.ret.tag === "Int64"
              ? dv.getBigInt64(ptr, true)
              : dv.getBigUint64(ptr, true);
          case "f32":
            return dv.getFloat32(ptr, true);
          case "f64":
            return dv.getFloat64(ptr, true);
          default:
            return undefined;
        }
      } finally {
        if (!entered)
          for (const { ptr, len } of allocated) ctx.free(ptr, len, 1);
        if (!entered || settled) ctx.free(frame, size, 8);
      }
    };
  }
}
