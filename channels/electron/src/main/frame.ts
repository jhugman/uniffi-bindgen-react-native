/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { WebContents, WebFrameMain } from "electron";
import { PORT, type FailureReason, type PortReply } from "../protocol.js";

/** The webContents a frame belongs to, as far as a send needs it. */
export type FrameOwner = Pick<WebContents, "isDestroyed" | "mainFrame">;

export function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** A detached frame is one a navigation replaced, unless it is still the main
 * frame: Electron leaves `detached` set on the main frame that a reload
 * re-hosts after a renderer crash, and a send to it lands. */
function isReachable(wc: FrameOwner, frame: WebFrameMain): boolean {
  if (frame.isDestroyed()) return false;
  return !frame.detached || (!wc.isDestroyed() && wc.mainFrame === frame);
}

/** Skips a frame that is gone, and passes a throwing send to warn. Returns
 * whether it sent. Every send from main to a page goes through here: a
 * navigation can dispose of the frame while its utility starts, and a send
 * that throws in an IPC handler, a utility event or a timer is an uncaught
 * exception in main. Electron's frame.send() logs a failure instead of
 * throwing, so for send() the result means only that the frame was there. */
export function sendToFrame(
  wc: FrameOwner,
  frame: WebFrameMain,
  warn: (message: string) => void,
  send: (frame: WebFrameMain) => void,
): boolean {
  try {
    if (!isReachable(wc, frame)) return false;
    send(frame);
    return true;
  } catch (e) {
    warn(`could not reach the page: ${messageOf(e)}`);
    return false;
  }
}

/** Rejects the page's connect() for `id`. */
export function sendFailure(
  wc: FrameOwner,
  frame: WebFrameMain,
  warn: (message: string) => void,
  id: string,
  reason: FailureReason,
  message: string,
): void {
  const reply: PortReply = { id, error: { reason, message } };
  sendToFrame(wc, frame, warn, (f) => f.postMessage(PORT, reply, []));
}
