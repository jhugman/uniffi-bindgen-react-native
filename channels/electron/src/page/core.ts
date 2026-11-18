/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  BRIDGE_KEY,
  DISCONNECTED,
  PORT,
  PROTOCOL,
  isOfType,
  type Bridge,
  type DisconnectedNotice,
  type PortReply,
} from "../protocol.js";
import {
  wrapPagePort,
  type DisconnectInfo,
  type PageChannelPort,
  type PagePort,
} from "./port.js";

const DEFAULT_TIMEOUT_MS = 30_000;

/** Why connect() failed. On any of them the app can fall back to wasm. */
export class ConnectError extends Error {
  readonly reason: "unavailable" | "not-served" | "start-failed" | "timeout";

  constructor(reason: ConnectError["reason"], message: string) {
    super(message);
    this.name = "ConnectError";
    this.reason = reason;
  }
}

export interface Connection {
  /** Drops transfer lists. Its `closed` is `disconnected`. */
  port: PageChannelPort;
  info: unknown;
  /** Settles once: the utility died, or the page closed the port. */
  disconnected: Promise<DisconnectInfo>;
}

export interface PageWindow {
  addEventListener(type: "message", listener: (ev: MessageEvent) => void): void;
  removeEventListener(
    type: "message",
    listener: (ev: MessageEvent) => void,
  ): void;
  __ubjsElectron?: Bridge;
}

/** What the preload posts to the page. */
type Incoming =
  | ({ type: typeof PORT } & PortReply)
  | ({ type: typeof DISCONNECTED } & DisconnectedNotice);

function isIncoming(data: unknown): data is Incoming {
  return (
    isOfType<Incoming>(data, PORT) || isOfType<Incoming>(data, DISCONNECTED)
  );
}

function newId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export function createPage(win: PageWindow): {
  isAvailable(): boolean;
  connect(options?: { timeoutMs?: number }): Promise<Connection>;
} {
  let connecting: Promise<Connection> | undefined;

  const isAvailable = () => win[BRIDGE_KEY]?.protocol === PROTOCOL;

  function start(timeoutMs: number): Promise<Connection> {
    const bridge = win[BRIDGE_KEY];
    if (bridge?.protocol !== PROTOCOL) {
      return Promise.reject(
        new ConnectError(
          "unavailable",
          `no @ubjs/electron bridge of protocol ${PROTOCOL} in this page; is @ubjs/electron/preload loaded, from the same release?`,
        ),
      );
    }
    const id = newId();
    let resolve!: (connection: Connection) => void;
    let reject!: (error: ConnectError) => void;
    const connected = new Promise<Connection>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    let waiting = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let pagePort: PagePort | undefined;

    const stop = () => {
      waiting = false;
      clearTimeout(timer);
    };
    const fail = (reason: ConnectError["reason"], message: string) => {
      if (!waiting) return;
      stop();
      reject(new ConnectError(reason, message));
    };

    /** Acts on a message for our id, and returns the port it keeps. */
    function deliver(
      message: Incoming,
      ports: readonly MessagePort[],
    ): MessagePort | undefined {
      if (message.type === DISCONNECTED) {
        if (pagePort) {
          pagePort.disconnect({ code: message.code, reason: message.reason });
        } else {
          fail(
            "start-failed",
            "the utility exited before the page got its port",
          );
        }
        return undefined;
      }
      if (!waiting) return undefined;
      if ("error" in message) {
        fail(message.error.reason, message.error.message);
        return undefined;
      }
      if (ports.length !== 1) return undefined;
      stop();
      pagePort = wrapPagePort(ports[0]);
      resolve({
        port: pagePort.port,
        info: message.info,
        disconnected: pagePort.port.closed,
      });
      return ports[0];
    }

    // Kept for the document's life, so a port that comes late is closed too.
    win.addEventListener("message", (ev) => {
      if (!isIncoming(ev.data)) return;
      let kept: MessagePort | undefined;
      try {
        if (ev.source === win && ev.data.id === id) {
          kept = deliver(ev.data, ev.ports);
        }
      } finally {
        for (const port of ev.ports) if (port !== kept) port.close();
      }
    });
    timer = setTimeout(
      () =>
        fail(
          "timeout",
          `no reply from the main process within ${timeoutMs} ms: main does not import @ubjs/electron/main, or the utility is still starting`,
        ),
      timeoutMs,
    );
    try {
      bridge.connect(id);
    } catch (e) {
      fail(
        "unavailable",
        `the bridge could not ask: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    return connected;
  }

  return {
    isAvailable,
    connect: ({ timeoutMs = DEFAULT_TIMEOUT_MS } = {}) =>
      (connecting ??= start(timeoutMs)),
  };
}
