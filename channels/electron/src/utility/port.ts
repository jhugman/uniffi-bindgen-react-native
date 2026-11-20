/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { MessagePortMain } from "electron";
import type { ChannelPort } from "@ubjs/worker";

/** Fits an Electron MessagePortMain, an EventEmitter, to ChannelPort.
 * MessagePortMain refuses anything but a port in a transfer list, so
 * messages are copied instead. `closed` settles on `close()`, or when the
 * far end goes; Electron reports that only once the port has started. */
export function fromMessagePortMain(port: MessagePortMain): ChannelPort & {
  start(): void;
  close(): void;
  readonly closed: Promise<void>;
} {
  let open = true;
  let settle!: () => void;
  const closed = new Promise<void>((resolve) => (settle = resolve));
  const end = (): void => {
    open = false;
    settle();
  };
  // Electron emits `close` when the far end disconnects.
  port.once("close", end);
  return {
    postMessage(message) {
      if (open) port.postMessage(message);
    },
    addEventListener(type, listener) {
      if (type === "message") port.on("message", listener);
    },
    removeEventListener(type, listener) {
      if (type === "message") port.off("message", listener);
    },
    start: () => port.start(),
    close() {
      end();
      port.close();
    },
    closed,
  };
}
