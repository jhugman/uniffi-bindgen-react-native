/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// JS-side registry for `RustFutureContinuationCallback`, UniFFI's poll-style
// async ABI.
//
//   * The continuation has a *fixed* `(data: u64, code: i8)` signature, so one
//     trampoline serves every future in the cdylib.
//   * `data` is **not** a UniFFI handle. It is an opaque u64 cookie the player
//     allocates per await and the host wasm passes back unchanged when the
//     future settles.
//   * Each await allocates a cookie and records `cookie -> resolver`. When the
//     trampoline fires, the continuation route in `CallbackTable.dispatch`
//     looks the resolver up by cookie rather than by the usual handle, and
//     calls it with `code`.

import type { CallbackTable } from "./callback.js";
import { FfiType } from "./ffi-type.js";

/** The single shape used by every `RustFutureContinuationCallback`. */
const CONT_DEF = {
  args: [FfiType.UInt64, FfiType.Int8],
  ret: FfiType.Void,
  hasRustCallStatus: false,
};

export class FutureRegistry {
  private nextData: bigint = 1n;
  private resolvers = new Map<bigint, (code: number) => void>();
  private contShapeId = -1;

  constructor(private callbacks: CallbackTable) {}

  /**
   * Compile (once) the continuation trampoline and install it at
   * `tableIndex` in the host's indirect function table. Subsequent calls
   * reuse the cached shape but re-install at whichever table index the
   * caller asks for. Returns `tableIndex` for convenience.
   */
  installContinuation(tableIndex: number): number {
    if (this.contShapeId < 0) {
      this.contShapeId = this.callbacks.defineShape(
        "__rust_future_continuation",
        CONT_DEF as any,
      );
      this.callbacks.registerContinuationShape(
        this.contShapeId,
        (data: bigint, code: number) => {
          const resolver = this.resolvers.get(data);
          if (!resolver) return;
          this.resolvers.delete(data);
          resolver(code);
        },
      );
    }
    this.callbacks.installAt(tableIndex, this.contShapeId);
    return tableIndex;
  }

  /**
   * Allocate a fresh continuation cookie and record the resolver to be
   * invoked when the host wasm calls the continuation trampoline with that
   * cookie. The returned `bigint` is the `data` argument the caller should
   * thread through to the Rust-side `rust_future_poll(...)`-style export.
   */
  allocateContinuation(resolver: (code: number) => void): bigint {
    const data = this.nextData++;
    this.resolvers.set(data, resolver);
    return data;
  }
}
