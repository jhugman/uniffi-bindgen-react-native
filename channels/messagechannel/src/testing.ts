/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ModuleDefinitions } from "@ubjs/core";
import type { ChannelListener, ChannelPort } from "./port.js";
import { compilePlan } from "./plan.js";
import { SenderCore } from "./sender.js";
import type { RegisteredPlayer } from "./types.js";

class SyncPort implements ChannelPort {
  peer!: SyncPort;
  private listeners: ChannelListener[] = [];
  private queue: unknown[] = [];

  postMessage(msg: unknown, _transfer?: Transferable[]): void {
    this.peer.deliver(msg);
  }

  addEventListener(_type: "message", listener: ChannelListener): void {
    this.listeners.push(listener);
    const queued = this.queue;
    this.queue = [];
    for (const m of queued) this.deliver(m);
  }

  removeEventListener(_type: "message", listener: ChannelListener): void {
    this.listeners = this.listeners.filter((l) => l !== listener);
  }

  close(): void {
    this.listeners = [];
    this.queue = [];
  }

  private deliver(msg: unknown): void {
    if (this.listeners.length === 0) {
      this.queue.push(msg);
      return;
    }
    for (const l of [...this.listeners]) l({ data: msg });
  }
}

/** Two ports that deliver to each other synchronously and by reference:
 * no structured clone, no transfer. For tests and the fixture harness. */
export function createSyncPortPair(): [ChannelPort, ChannelPort] {
  const a = new SyncPort();
  const b = new SyncPort();
  a.peer = b;
  b.peer = a;
  return [a, b];
}

/** A player-shaped object whose every function returns synchronously. Only
 * works over a port from `createSyncPortPair`; anything else throws. */
export function createSyncPlayer(
  defs: ModuleDefinitions,
  port: ChannelPort,
): RegisteredPlayer & { close(): void } {
  const core = new SenderCore(compilePlan(defs), port);
  const player: Record<string, unknown> = Object.create(null);
  for (const name of Object.keys(defs.functions)) {
    player[name] = (...args: unknown[]) => core.invokeSync(name, args);
  }
  player.rustbuffer_alloc = (n: number) => core.rustbuffer_alloc(n);
  player.rustbuffer_free = (v: Uint8Array) => core.rustbuffer_free(v);
  player.close = () => core.close();
  return player as unknown as RegisteredPlayer & { close(): void };
}
