/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ContextBridge, IpcRenderer, IpcRendererEvent } from "electron";
import {
  BRIDGE_KEY,
  CONNECT,
  DISCONNECTED,
  PORT,
  PROTOCOL,
  isConnectionId,
  type Bridge,
  type ConnectRequest,
} from "../protocol.js";

export interface PreloadDeps {
  ipcRenderer: Pick<IpcRenderer, "send" | "on">;
  contextBridge: Pick<ContextBridge, "exposeInMainWorld">;
  window: Pick<Window, "postMessage">;
  contextIsolated: boolean;
}

function isFor(
  payload: unknown,
  id: string | undefined,
): payload is { id: string } {
  return (
    id !== undefined &&
    typeof payload === "object" &&
    payload !== null &&
    (payload as { id?: unknown }).id === id
  );
}

function closeAll(ports: readonly MessagePort[]): void {
  for (const port of ports) port.close();
}

export function installPreload(deps: PreloadDeps): void {
  const { ipcRenderer, contextBridge, window } = deps;
  // Without isolation the page shares this world, and with it ipcRenderer.
  if (!deps.contextIsolated) {
    throw new Error(
      "@ubjs/electron/preload needs contextIsolation, so that the page cannot reach ipcRenderer",
    );
  }

  // The latest connect()'s id. It covers the port reply and every later notice for that
  // connection, so a reply delayed across a reload is dropped.
  let pending: string | undefined;

  const bridge: Bridge = {
    protocol: PROTOCOL,
    connect(id: string) {
      if (!isConnectionId(id)) {
        throw new TypeError(
          "connect() needs an id of 32 lowercase hex characters",
        );
      }
      pending = id;
      const request: ConnectRequest = { id };
      ipcRenderer.send(CONNECT, request);
    },
  };
  // Exposed before listening: a second copy of this preload throws here, so it
  // installs nothing and never acts on IPC.
  contextBridge.exposeInMainWorld(BRIDGE_KEY, bridge);

  ipcRenderer.on(PORT, (event: IpcRendererEvent, payload: unknown) => {
    if (isFor(payload, pending)) {
      // `type` last, so a payload cannot change it.
      window.postMessage({ ...payload, type: PORT }, "*", event.ports);
    } else {
      closeAll(event.ports);
    }
  });
  ipcRenderer.on(DISCONNECTED, (event: IpcRendererEvent, payload: unknown) => {
    closeAll(event.ports);
    if (isFor(payload, pending)) {
      window.postMessage({ ...payload, type: DISCONNECTED }, "*");
    }
  });
}
