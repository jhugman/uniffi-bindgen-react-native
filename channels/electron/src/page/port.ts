/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ChannelPort } from "@ubjs/worker";

export interface DisconnectInfo {
  code: number | null;
  reason: string;
}

/** A ChannelPort whose start(), close() and closed are always there. */
export type PageChannelPort = ChannelPort & {
  start(): void;
  close(): void;
  readonly closed: Promise<DisconnectInfo>;
};

export interface PagePort {
  /** Drops transfer lists. Its `closed` settles with the first disconnect. */
  readonly port: PageChannelPort;
  /** The first call wins: closes the raw port, settles `closed`; later calls do nothing. */
  disconnect(info: DisconnectInfo): void;
}

/** Fits the page's end of the utility's port to ChannelPort. A buffer in a
 * transfer list to a MessagePortMain is detached and the whole message lands
 * as null, so messages are copied instead. */
export function wrapPagePort(raw: MessagePort): PagePort {
  let open = true;
  let settle!: (info: DisconnectInfo) => void;
  const closed = new Promise<DisconnectInfo>((resolve) => (settle = resolve));
  const disconnect = (info: DisconnectInfo): void => {
    if (!open) return;
    open = false;
    raw.close();
    // Copied, so a notice passed whole settles as just its code and reason.
    settle({ code: info.code, reason: info.reason });
  };
  return {
    port: {
      // A no-op after a disconnect, so a closing sender can still post its cancels.
      postMessage(message) {
        if (open) raw.postMessage(message);
      },
      addEventListener: (type, listener) =>
        raw.addEventListener(type, listener),
      removeEventListener: (type, listener) =>
        raw.removeEventListener(type, listener),
      start: () => raw.start(),
      close: () => disconnect({ code: null, reason: "closed" }),
      closed,
    },
    disconnect,
  };
}
