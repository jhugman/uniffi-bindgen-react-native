/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { PreloadDeps } from "../../src/preload/core.js";

/** A port that only records whether it was closed. */
export interface StubPort {
  closed: boolean;
  start(): void;
  postMessage(message: unknown): void;
  addEventListener(type: string, listener: unknown): void;
  removeEventListener(type: string, listener: unknown): void;
  close(): void;
}

export function stubPort(): StubPort {
  const port: StubPort = {
    closed: false,
    start() {},
    postMessage() {},
    addEventListener() {},
    removeEventListener() {},
    close() {
      port.closed = true;
    },
  };
  return port;
}

type IpcListener = (event: { ports: StubPort[] }, payload: unknown) => void;

/** The preload's Electron objects, recording what it exposes, sends and posts. */
export function fakeRenderer(contextIsolated = true) {
  const exposed = new Map<string, any>();
  const sent: [string, unknown][] = [];
  const posted: { message: unknown; origin: string; ports: unknown[] }[] = [];
  const listeners = new Map<string, IpcListener[]>();

  const ipcRenderer = {
    send(channel: string, payload: unknown) {
      sent.push([channel, payload]);
    },
    on(channel: string, listener: IpcListener) {
      listeners.set(channel, [...(listeners.get(channel) ?? []), listener]);
      return ipcRenderer;
    },
  };
  const deps = {
    ipcRenderer,
    contextBridge: {
      exposeInMainWorld(key: string, api: unknown) {
        // Electron's own message.
        if (exposed.has(key))
          throw new Error(
            "Cannot bind an API on top of an existing property on the window object",
          );
        exposed.set(key, api);
      },
    },
    window: {
      postMessage(message: unknown, origin: string, ports?: unknown[]) {
        posted.push({ message, origin, ports: ports ?? [] });
      },
    },
    contextIsolated,
  } as unknown as PreloadDeps;

  return {
    deps,
    exposed,
    sent,
    posted,
    deliver(channel: string, payload: unknown, ports: StubPort[] = []) {
      for (const listener of listeners.get(channel) ?? [])
        listener({ ports }, payload);
    },
    listenerCount: () =>
      [...listeners.values()].reduce((n, l) => n + l.length, 0),
  };
}
