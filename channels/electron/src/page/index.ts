/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { Bridge } from "../protocol.js";
import { ConnectError, createPage, type Connection } from "./core.js";
import type { DisconnectInfo } from "./port.js";

declare global {
  interface Window {
    /** Put there by @ubjs/electron/preload. */
    __ubjsElectron?: Bridge;
  }
}

let page: ReturnType<typeof createPage> | undefined;

// Made on first use, so importing this module touches no window.
const shared = () => (page ??= createPage(window));

/** Whether the preload is here, so the page can ask. connect() is the real
 * answer: it rejects with `not-served` when main does not serve this window. */
export function isAvailable(): boolean {
  return shared().isAvailable();
}

/** Asks main for the library's port. Resolves once the utility has opened the
 * library; rejects with a ConnectError, on which the app can fall back to
 * wasm. Memoized: every call gets the first call's promise, so a rejection
 * lasts until the page reloads. `timeoutMs` defaults to 30 000; Infinity
 * waits forever. Pass `connection.port` to the generated `uniffiInitAsync`. */
export function connect(options?: { timeoutMs?: number }): Promise<Connection> {
  return shared().connect(options);
}

export { ConnectError };
export type { Connection, DisconnectInfo };
