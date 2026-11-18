/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { EventEmitter } from "node:events";
import type { MessagePortMain } from "electron";

/** What one end of a fake crossing calls on the other. */
interface FakeEnd {
  isOpen(): boolean;
  receive(data: unknown, ports: unknown[]): void;
  remoteClosed(): void;
}

// A port delivers on a later turn. A message already posted still lands,
// unless its receiver has closed by then.
function deliverLater(to: FakeEnd, data: unknown, ports: unknown[]): void {
  setImmediate(() => {
    if (to.isOpen()) to.receive(data, ports);
  });
}

function closeLater(peer: FakeEnd | undefined): void {
  if (peer)
    setImmediate(() => {
      if (peer.isOpen()) peer.remoteClosed();
    });
}

/** Electron's transfer lists in main hold only MessagePortMains. */
export function checkPorts(transfer: readonly unknown[]): void {
  transfer.forEach((t, i) => {
    if (!(t instanceof FakePortMain))
      throw new TypeError(`Port at index ${i} is not a valid port`);
  });
}

/** Holds deliveries until start(), as both kinds of port do. */
class Inbox<E> {
  private queue: E[] = [];
  private started = false;
  constructor(private dispatch: (ev: E) => void) {}
  push(ev: E): void {
    this.queue.push(ev);
    if (this.started) this.drain();
  }
  start(): void {
    if (this.started) return;
    this.started = true;
    setImmediate(() => this.drain());
  }
  clear(): void {
    this.queue = [];
  }
  private drain(): void {
    while (this.queue.length > 0) this.dispatch(this.queue.shift()!);
  }
}

/** Electron's MessagePortMain: a transfer list may hold only ports, and only
 * the far end hears `close`. */
export class FakePortMain extends EventEmitter implements FakeEnd {
  peer: FakeEnd | undefined;
  closed = false;
  /** postMessage calls that did not throw, delivered or not. */
  posted = 0;
  private inbox = new Inbox<{ data: unknown; ports: unknown[] }>((ev) =>
    this.emit("message", ev),
  );

  start(): void {
    this.inbox.start();
  }

  postMessage(message: unknown, transfer: readonly unknown[] = []): void {
    checkPorts(transfer);
    const data = structuredClone(message);
    this.posted++;
    if (this.closed || !this.peer?.isOpen()) return;
    deliverLater(this.peer, data, [...transfer]);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.inbox.clear();
    closeLater(this.peer);
  }

  isOpen(): boolean {
    return !this.closed;
  }
  receive(data: unknown, ports: unknown[]): void {
    this.inbox.push({ data, ports });
  }
  remoteClosed(): void {
    this.emit("close");
  }
}

/** A fake port, typed as the Electron port it stands in for. */
export const asMain = (p: FakePortMain) => p as unknown as MessagePortMain;

export function fakeMessageChannelMain(): {
  port1: FakePortMain;
  port2: FakePortMain;
} {
  const port1 = new FakePortMain();
  const port2 = new FakePortMain();
  port1.peer = port2;
  port2.peer = port1;
  return { port1, port2 };
}

type PageListener = (ev: { data: unknown }) => void;

/** The renderer's DOM MessagePort to a MessagePortMain: a non-empty transfer
 * list detaches its buffers, and the whole message lands as null. */
export class FakePagePort implements FakeEnd {
  peer: FakeEnd | undefined;
  // Not `closed`: ChannelPort types that member as a promise.
  isClosed = false;
  /** Every postMessage call, delivered or not. */
  posted = 0;
  private listeners = new Set<PageListener>();
  private inbox = new Inbox<{ data: unknown }>((ev) => {
    for (const listener of [...this.listeners]) listener(ev);
  });

  addEventListener(type: string, listener: PageListener): void {
    if (type === "message") this.listeners.add(listener);
  }
  removeEventListener(type: string, listener: PageListener): void {
    if (type === "message") this.listeners.delete(listener);
  }
  start(): void {
    this.inbox.start();
  }

  postMessage(message: unknown, transfer: readonly unknown[] = []): void {
    this.posted++;
    let data: unknown = null;
    if (transfer.length > 0)
      structuredClone(transfer, { transfer: transfer as Transferable[] });
    else data = structuredClone(message);
    if (this.isClosed || !this.peer?.isOpen()) return;
    deliverLater(this.peer, data, []);
  }

  close(): void {
    if (this.isClosed) return;
    this.isClosed = true;
    this.inbox.clear();
    closeLater(this.peer);
  }

  isOpen(): boolean {
    return !this.isClosed;
  }
  receive(data: unknown): void {
    this.inbox.push({ data });
  }
  // Nothing relies on a DOM port's close event.
  remoteClosed(): void {}
}

/** The page's port and the utility's, entangled across the process boundary. */
export function fakeRendererToUtility(): {
  page: FakePagePort;
  utility: FakePortMain;
} {
  const page = new FakePagePort();
  const utility = new FakePortMain();
  page.peer = utility;
  utility.peer = page;
  return { page, utility };
}
