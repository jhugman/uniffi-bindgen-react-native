/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// What main, the utility, the preload and the page agree on. No Node or DOM
// API here: three tsc projects compile this file.

export const CONNECT = "ubjs-electron:v1:connect";
export const PORT = "ubjs-electron:v1:port";
export const DISCONNECTED = "ubjs-electron:v1:disconnected";
export const INIT = "ubjs-electron:init";
export const READY = "ubjs-electron:ready";
export const ERROR = "ubjs-electron:error";
export const BRIDGE_KEY = "__ubjsElectron";
export const PROTOCOL = 1;

export type FailureReason = "not-served" | "start-failed";

export interface ConnectRequest {
  id: string;
}

export type PortReply =
  | { id: string; info: unknown }
  | { id: string; error: { reason: FailureReason; message: string } };

export interface DisconnectedNotice {
  id: string;
  code: number;
  reason: "utility-exited";
}

export interface InitMessage {
  type: typeof INIT;
  id: string;
  library: string;
  info: unknown;
}

export interface ReadyMessage {
  type: typeof READY;
}

export interface ErrorMessage {
  type: typeof ERROR;
  message: string;
  stack?: string;
}

/** What the preload puts on `window[BRIDGE_KEY]`. */
export interface Bridge {
  readonly protocol: typeof PROTOCOL;
  connect(id: string): void;
}

export function isConnectionId(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{32}$/.test(value);
}

export function isOfType<T extends { type: string }>(
  message: unknown,
  type: T["type"],
): message is T {
  return (
    typeof message === "object" &&
    message !== null &&
    (message as { type?: unknown }).type === type
  );
}
