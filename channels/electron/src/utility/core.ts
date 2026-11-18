/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { MessagePortMain, ParentPort } from "electron";
import type { ModuleDefinitions } from "@ubjs/core";
import type { UniffiNativeModule } from "@ubjs/node";
import { createReceiver } from "@ubjs/worker";
import {
  ERROR,
  INIT,
  READY,
  isOfType,
  type ErrorMessage,
  type InitMessage,
  type ReadyMessage,
} from "../protocol.js";
import { fromMessagePortMain } from "./port.js";

/** What onInit sees of main's init message. */
export interface UtilityInit {
  library: string;
  /** The app's `info`, the same value the page gets. */
  info: unknown;
}

export interface ServeUtilityOptions {
  /** Runs before the library opens, e.g. to set up logging or read `info`. */
  onInit?(init: UtilityInit): void;
}

export interface UtilityDeps {
  parentPort: Pick<ParentPort, "on" | "postMessage">;
  exit(code: number): void;
  open(
    library: string,
  ):
    | Pick<UniffiNativeModule, "register">
    | Promise<Pick<UniffiNativeModule, "register">>;
}

function closeAll(ports: readonly MessagePortMain[]): void {
  for (const port of ports) port.close();
}

function errorMessage(e: unknown): ErrorMessage {
  if (e instanceof Error) {
    return e.stack === undefined
      ? { type: ERROR, message: e.message }
      : { type: ERROR, message: e.message, stack: e.stack };
  }
  return { type: ERROR, message: String(e) };
}

export function serveUtilityWith(
  deps: UtilityDeps,
  definitions: ModuleDefinitions,
  options: ServeUtilityOptions = {},
): void {
  const { parentPort, exit, open } = deps;
  let initialized = false;

  const start = async (init: InitMessage, port: MessagePortMain) => {
    options.onInit?.({ library: init.library, info: init.info });
    const mod = await open(init.library);
    // Once only: a second register resolves every symbol again.
    const player = mod.register(definitions);
    const channel = fromMessagePortMain(port);
    const receiver = createReceiver(definitions, player, channel);
    const ready: ReadyMessage = { type: READY };
    parentPort.postMessage(ready);

    // The page is gone. No unload: it would leave an async call in flight pending.
    channel.closed.then(() => {
      receiver.close();
      exit(0);
    });
  };

  // Never removed: the listener keeps the utility up. parentPort holds main's
  // init until it is attached.
  parentPort.on("message", (event) => {
    if (!isOfType<InitMessage>(event.data, INIT)) return;
    if (initialized) {
      closeAll(event.ports);
      return;
    }
    initialized = true;
    start(event.data, event.ports[0]).catch((e: unknown) => {
      closeAll(event.ports);
      parentPort.postMessage(errorMessage(e));
      exit(1);
    });
  });
}
