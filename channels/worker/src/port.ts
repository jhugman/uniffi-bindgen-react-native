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
}
