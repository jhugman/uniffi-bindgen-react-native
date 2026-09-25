/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { UniffiRustCallStatus } from "@ubjs/core";

export interface WireError {
  name: string;
  message: string;
}

export type CallMessage = {
  kind: "call";
  id: number;
  fn: string;
  args: unknown[];
};
export type ReturnMessage =
  | {
      kind: "return";
      id: number;
      ok: true;
      value: unknown;
      status?: UniffiRustCallStatus;
    }
  | { kind: "return"; id: number; ok: false; error: WireError };
export type CallbackMessage = {
  kind: "callback";
  id: number;
  cb: number;
  args: unknown[];
};
export type CallbackReturnMessage =
  | { kind: "callback-return"; id: number; ok: true; value: unknown }
  | { kind: "callback-return"; id: number; ok: false; error: WireError };
export type ReleaseMessage = { kind: "release"; cb: number };

export type ChannelMessage =
  | CallMessage
  | ReturnMessage
  | CallbackMessage
  | CallbackReturnMessage
  | ReleaseMessage;

const KINDS = new Set([
  "call",
  "return",
  "callback",
  "callback-return",
  "release",
]);

export function isChannelMessage(x: unknown): x is ChannelMessage {
  return (
    typeof x === "object" &&
    x !== null &&
    typeof (x as { kind?: unknown }).kind === "string" &&
    KINDS.has((x as { kind: string }).kind)
  );
}

export function toWireError(e: unknown): WireError {
  if (e instanceof Error) return { name: e.name, message: e.message };
  return { name: "Error", message: String(e) };
}

export function fromWireError(w: WireError): Error {
  const e = new Error(w.message);
  e.name = w.name;
  return e;
}
