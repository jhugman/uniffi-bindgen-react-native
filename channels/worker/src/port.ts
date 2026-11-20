/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export interface ChannelMessageEvent {
  data: unknown;
}
export type ChannelListener = (ev: ChannelMessageEvent) => void;

/** The subset of `MessagePort` the channel uses. Parameters are widened
 * (`readonly unknown[]`, `(ev: any) => void`) so both Node's worker_threads
 * MessagePort and DOM/RN's MessagePort are assignable with no cast — their
 * declared `Transferable`/event types differ from each other and from ours. */
export interface ChannelPort {
  postMessage(msg: unknown, transfer?: readonly unknown[]): void;
  addEventListener(type: "message", listener: (ev: any) => void): void;
  removeEventListener(type: "message", listener: (ev: any) => void): void;
  start?(): void;
  close?(): void;
  /** Node's MessagePort: hold the event loop open. Absent on browser ports. */
  ref?(): void;
  unref?(): void;
  /** Settles once the far end is gone; a sender or receiver over this port then closes itself. */
  closed?: PromiseLike<unknown>;
}

/** Calls `close` once `port.closed` settles either way. Only a thenable
 * counts, so an adapter's boolean `closed` flag is ignored. */
export function onPortClosed(port: ChannelPort, close: () => void): void {
  const { closed } = port;
  if (typeof closed?.then !== "function") return;
  // Nothing awaits this reaction, so a throw from close() is warned, not left unhandled.
  const run = () => {
    try {
      close();
    } catch (e) {
      console.warn("worker: closing after the port closed threw", e);
    }
  };
  closed.then(run, run);
}
