/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type {
  FfiTypeDesc,
  ModuleDefinitions,
  UniffiRustCallStatus,
} from "@ubjs/core";

export type JsValueOf<T extends FfiTypeDesc> = T extends {
  tag: "UInt64" | "Int64" | "Handle";
}
  ? bigint
  : T extends { tag: "RustBuffer" }
    ? Uint8Array
    : T extends { tag: "Void" }
      ? void
      : T extends { tag: "Callback" }
        ? (...args: any[]) => any
        : T extends { tag: "Struct" | "Reference" | "MutReference" }
          ? object
          : number;

// A tuple (from an `as const` table) keeps arity; the bindgen's plain arrays
// widen to the union of the function's argument types.
type ArgsOf<A> = A extends readonly []
  ? []
  : A extends readonly [infer H extends FfiTypeDesc, ...infer R]
    ? [JsValueOf<H>, ...ArgsOf<R>]
    : A extends readonly (infer E extends FfiTypeDesc)[]
      ? JsValueOf<E>[]
      : never;

type WithStatus<Args extends unknown[], S> = S extends true
  ? [...Args, UniffiRustCallStatus]
  : Args;

// An async function's entry resolves with the completed value, not the
// future handle in `ret`, and always takes a status and then a signal.
type RetOf<F> = F extends { async: { ret: infer R extends FfiTypeDesc } }
  ? JsValueOf<R>
  : F extends { ret: infer R extends FfiTypeDesc }
    ? JsValueOf<R>
    : never;

type CallArgs<F> = F extends { async: object; args: infer A }
  ? [...ArgsOf<A>, UniffiRustCallStatus, AbortSignal | undefined]
  : F extends { args: infer A; hasRustCallStatus: infer S }
    ? WithStatus<ArgsOf<A>, S>
    : never;

export type AsyncPlayer<D extends ModuleDefinitions> = {
  [K in keyof D["functions"]]: (
    ...args: CallArgs<D["functions"][K]>
  ) => Promise<RetOf<D["functions"][K]>>;
};

// A type alias, not an interface: only an alias gets an implicit index
// signature, which a Sender needs to satisfy setNativeModule's Record.
export type SenderControl = {
  close(): void;
  rustbuffer_alloc(n: number): Uint8Array;
  rustbuffer_free(view: Uint8Array): void;
};

export type Sender<D extends ModuleDefinitions> = AsyncPlayer<D> &
  SenderControl;

/** What wasm2's `registerSync` returns. */
export type RegisteredPlayer = Record<string, (...args: any[]) => any> & {
  rustbuffer_alloc(n: number): Uint8Array;
  rustbuffer_free(view: Uint8Array): void;
};

export interface Receiver {
  close(): void;
}

export class ChannelClosedError extends Error {
  constructor(message = "worker: the channel is closed") {
    super(message);
    this.name = "ChannelClosedError";
  }
}
