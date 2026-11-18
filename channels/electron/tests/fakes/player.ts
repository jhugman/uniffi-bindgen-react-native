/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { FfiType, type ModuleDefinitions } from "@ubjs/core";
import type { RegisteredPlayer } from "@ubjs/worker";

export const DEFS = {
  symbols: {
    rustbuffer_alloc: "a",
    rustbuffer_free: "f",
    rustbuffer_from_bytes: "b",
  },
  functions: {
    echo: {
      args: [FfiType.RustBuffer],
      ret: FfiType.RustBuffer,
      hasRustCallStatus: false,
    },
    make_bytes: { args: [], ret: FfiType.RustBuffer, hasRustCallStatus: false },
    sleep: {
      args: [FfiType.UInt32],
      ret: FfiType.Handle,
      hasRustCallStatus: false,
      async: {
        poll: "p",
        cancel: "c",
        complete: "k",
        free: "f",
        ret: FfiType.UInt32,
      },
    },
  },
  callbacks: {},
  structs: {},
} satisfies ModuleDefinitions;

/** A player for DEFS that counts what it cancels and frees. */
export function fakePlayer(): {
  player: RegisteredPlayer;
  state: { cancelled: number; freed: number };
} {
  const state = { cancelled: 0, freed: 0 };
  const player: RegisteredPlayer = {
    rustbuffer_alloc: (n) => new Uint8Array(n),
    rustbuffer_free: () => {
      state.freed += 1;
    },
    echo: (v: Uint8Array) => v.slice(),
    make_bytes: () => new Uint8Array(1024).fill(7),
    // Resolves ms after ms; an abort settles it at once as cancelled (3).
    sleep: (
      ms: number,
      status: { code: number },
      signal: AbortSignal | undefined,
    ) =>
      new Promise((resolve) => {
        const onAbort = () => {
          clearTimeout(timer);
          state.cancelled += 1;
          status.code = 3;
          resolve(0);
        };
        const timer = setTimeout(() => {
          signal?.removeEventListener("abort", onAbort);
          status.code = 0;
          resolve(ms);
        }, ms);
        signal?.addEventListener("abort", onAbort);
      }),
  };
  return { player, state };
}
