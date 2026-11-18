/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { EventEmitter } from "node:events";
import type { UniffiNativeModule } from "@ubjs/node";
import type { RegisteredPlayer } from "@ubjs/worker";
import { INIT, type InitMessage } from "../../src/protocol.js";
import { ID } from "./helpers.js";

/** A utility's process.parentPort: records what it posts, cloned as Electron would. */
export class FakeParentPort extends EventEmitter {
  posted: unknown[] = [];

  postMessage(message: unknown): void {
    this.posted.push(structuredClone(message));
  }

  /** Main's init message, as the utility's listener receives it. */
  sendInit(init: Partial<InitMessage>, ports: unknown[]): void {
    const data = {
      type: INIT,
      id: ID,
      library: "/abs/lib.dylib",
      info: null,
      ...init,
    };
    this.emit("message", { data, ports });
  }
}

/** An opener for @ubjs/node that hands back `player` and counts what it does. */
export function fakeOpen(player: RegisteredPlayer): {
  open: (library: string) => Pick<UniffiNativeModule, "register">;
  opened: string[];
  registers: () => number;
} {
  const opened: string[] = [];
  let registers = 0;
  const open = (library: string) => {
    opened.push(library);
    return {
      register() {
        registers += 1;
        return player;
      },
      // An unload with an async call in flight leaves that call pending.
      unload() {
        throw new Error("the utility must not unload the library");
      },
    };
  };
  return { open, opened, registers: () => registers };
}
