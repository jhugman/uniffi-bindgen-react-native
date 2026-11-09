/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { FfiType, type ModuleDefinitions } from "@ubjs/core";
import type { RegisteredPlayer } from "../src/types.js";

export const DEFS = {
  symbols: {
    rustbuffer_alloc: "a",
    rustbuffer_free: "f",
    rustbuffer_from_bytes: "b",
  },
  functions: {
    add: {
      args: [FfiType.UInt32, FfiType.UInt32],
      ret: FfiType.UInt32,
      hasRustCallStatus: true,
    },
    fail: { args: [], ret: FfiType.Void, hasRustCallStatus: true },
    make_bytes: { args: [], ret: FfiType.RustBuffer, hasRustCallStatus: false },
    init_vtable: {
      args: [FfiType.Reference(FfiType.Struct("VT"))],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    fire: {
      args: [FfiType.Handle],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    clone_twice_free_thrice: {
      args: [FfiType.Handle],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    start_async: {
      args: [FfiType.Handle],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    drop_async: { args: [], ret: FfiType.Void, hasRustCallStatus: false },
    sync_method: {
      args: [FfiType.Handle],
      ret: FfiType.Int8,
      hasRustCallStatus: false,
    },
    init_vtable2: {
      args: [FfiType.Reference(FfiType.Struct("VT2"))],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    cross_vtable_refcount: {
      args: [FfiType.Handle],
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
    Clone: {
      args: [FfiType.Handle],
      ret: FfiType.Handle,
      hasRustCallStatus: false,
    },
    Notify: {
      args: [FfiType.Handle, FfiType.RustBuffer],
      ret: FfiType.Void,
      hasRustCallStatus: false,
    },
    AsyncM: {
      args: [FfiType.Handle, FfiType.Callback("Complete"), FfiType.Handle],
      ret: FfiType.Struct("Dropped"),
      hasRustCallStatus: false,
      outReturn: true,
    },
    SyncM: {
      args: [FfiType.Handle],
      ret: FfiType.Int8,
      hasRustCallStatus: true,
      outReturn: true,
    },
    Complete: {
      args: [FfiType.Handle, FfiType.Int32],
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
    VT: [
      { name: "uniffi_free", type: FfiType.Callback("Free") },
      { name: "uniffi_clone", type: FfiType.Callback("Clone") },
      { name: "notify", type: FfiType.Callback("Notify") },
      { name: "async_m", type: FfiType.Callback("AsyncM") },
      { name: "sync_m", type: FfiType.Callback("SyncM") },
    ],
    Dropped: [
      { name: "handle", type: FfiType.Handle },
      { name: "free", type: FfiType.Callback("DroppedFree") },
    ],
    // A second callback interface, to prove refcounts don't collide across
    // interfaces whose UniffiHandleMaps both start counting from 1n.
    VT2: [
      { name: "uniffi_free", type: FfiType.Callback("Free") },
      { name: "uniffi_clone", type: FfiType.Callback("Clone") },
      { name: "notify", type: FfiType.Callback("Notify") },
    ],
  },
} satisfies ModuleDefinitions;

/** A player that records what it is handed and lets the test poke its vtable. */
export function fakePlayer() {
  const state = {
    vtable: undefined as any,
    vtable2: undefined as any,
    freed: [] as Uint8Array[],
    completions: [] as unknown[],
    dropped: undefined as any,
  };
  const player: RegisteredPlayer = {
    rustbuffer_alloc: (n) => new Uint8Array(n),
    rustbuffer_free: (v) => {
      state.freed.push(v);
    },
    add: (a: number, b: number, status: { code: number }) => {
      status.code = 0;
      return a + b;
    },
    fail: (status: { code: number; errorBuf?: Uint8Array }) => {
      status.code = 1;
      status.errorBuf = new Uint8Array([7]);
    },
    make_bytes: () => new Uint8Array([1, 2, 3]),
    init_vtable: (vt: any) => {
      state.vtable = vt;
    },
    fire: (h: bigint) =>
      state.vtable.notify(
        h,
        new Uint8Array([h === 0n ? 0 : Number(h & 0xffn)]),
      ),
    clone_twice_free_thrice: (h: bigint) => {
      assert.strictEqual(state.vtable.uniffi_clone(h), h);
      assert.strictEqual(state.vtable.uniffi_clone(h), h);
      state.vtable.uniffi_free(h);
      state.vtable.uniffi_free(h);
      state.vtable.uniffi_free(h);
    },
    start_async: (h: bigint) => {
      const complete = (data: bigint, v: number) =>
        state.completions.push([data, v]);
      state.dropped = state.vtable.async_m(h, complete, 42n);
    },
    drop_async: () => state.dropped.free(state.dropped.handle),
    sync_method: (h: bigint) => state.vtable.sync_m(h),
    init_vtable2: (vt: any) => {
      state.vtable2 = vt;
    },
    cross_vtable_refcount: (h: bigint) => {
      state.vtable.uniffi_clone(h);
      state.vtable2.uniffi_free(h);
      state.vtable.uniffi_free(h);
      state.vtable.uniffi_free(h);
    },
  };
  return { player, state };
}
