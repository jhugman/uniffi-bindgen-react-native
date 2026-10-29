/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// An async function as one entry the caller awaits. The player runs UniFFI's
// poll loop here, so a caller behind a channel pays one round trip per await
// rather than one per poll.
import {
  uniffiDriveRustFuture,
  type AsyncFunctionDef,
  type FunctionDef,
  type UniffiRustCallStatus,
  type UniffiRustFutureFns,
} from "@ubjs/core";
import {
  copyAndFreeRustBufferView,
  specializeExport,
  type DispatchContext,
} from "./call.js";
import { FfiType } from "./ffi-type.js";
import { CONT_DEF, RUST_FUTURE_CONTINUATION } from "./future.js";

// UniFFI's future exports have a fixed ABI, so these shapes are not in the
// table.
const POLL_DEF: FunctionDef = {
  args: [
    FfiType.Handle,
    FfiType.Callback(RUST_FUTURE_CONTINUATION),
    FfiType.Handle,
  ],
  ret: FfiType.Void,
  hasRustCallStatus: false,
};
const HANDLE_ONLY_DEF: FunctionDef = {
  args: [FfiType.Handle],
  ret: FfiType.Void,
  hasRustCallStatus: false,
};

export function hasAsyncEntry(
  def: FunctionDef,
): def is FunctionDef & { async: AsyncFunctionDef } {
  return Boolean(def.async);
}

/** `poll` takes the continuation as a `Callback` arg. UniFFI fixes that
 * callback's shape, so the player's own wins over any shape the table
 * carries under the same name. */
function specializePoll(
  ctx: DispatchContext,
  exports: WebAssembly.Exports,
  name: string,
): (...args: any[]) => any {
  // Async functions share poll exports: copy the callbacks only on a miss.
  const cached = ctx.specialized.get(name);
  if (cached) return cached;
  const callbackDefs = new Map(ctx.callbackDefs);
  callbackDefs.set(RUST_FUTURE_CONTINUATION, CONT_DEF);
  return specializeExport({ ...ctx, callbackDefs }, exports, name, POLL_DEF);
}

/** Registers a future-starting export as one Promise-returning entry:
 * `(...args, status, signal)` starts the future synchronously (lowered
 * RustBuffer args alias wasm memory and must be consumed before any
 * await), drives it to READY, completes into `status`, frees it, and
 * resolves with the lowered value — a RustBuffer result as an owned copy
 * with its wasm allocation already freed. */
export function buildAsyncEntry(
  ctx: DispatchContext,
  exports: WebAssembly.Exports,
  fnName: string,
  def: FunctionDef & { async: AsyncFunctionDef },
): (...args: any[]) => Promise<unknown> {
  const start = specializeExport(ctx, exports, fnName, def);
  // `complete` is synchronous, so sharing its scratch is safe.
  const complete = specializeExport(ctx, exports, def.async.complete, {
    args: [FfiType.Handle],
    ret: def.async.ret,
    hasRustCallStatus: true,
  });
  const fns: UniffiRustFutureFns<unknown, UniffiRustCallStatus> = {
    poll: specializePoll(ctx, exports, def.async.poll),
    cancel: specializeExport(ctx, exports, def.async.cancel, HANDLE_ONLY_DEF),
    free: specializeExport(ctx, exports, def.async.free, HANDLE_ONLY_DEF),
    complete: (rustFuture, status) => {
      const value = complete(rustFuture, status);
      // A view over wasm memory would outlive this call, and the next wasm
      // memory growth would detach it: hand back a copy and free the original.
      return value instanceof Uint8Array
        ? copyAndFreeRustBufferView(ctx, value)
        : value;
    },
  };
  const argCount = def.args.length;
  return function asyncEntry(...args: any[]): Promise<unknown> {
    const status = args[argCount] as UniffiRustCallStatus;
    const signal = args[argCount + 1] as AbortSignal | undefined;
    // Before any await: lowered RustBuffer args alias wasm memory.
    const rustFuture = start(...args.slice(0, argCount)) as bigint;
    return uniffiDriveRustFuture(rustFuture, fns, status, signal);
  };
}
