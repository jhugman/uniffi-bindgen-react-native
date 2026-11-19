/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { isAbsolute } from "node:path";
import type {
  ForkOptions,
  IpcMain,
  IpcMainEvent,
  MessagePortMain,
  UtilityProcess,
  WebContents,
  WebFrameMain,
} from "electron";
import { CONNECT, isConnectionId, type FailureReason } from "../protocol.js";
import { messageOf, sendFailure, type FrameOwner } from "./frame.js";
import { inspectArgv, unpackedPath } from "./paths.js";
import {
  CLOSED,
  REPLACED,
  UtilityConnection,
  type ConnectionContext,
} from "./utility-connection.js";

export interface ServeOptions {
  /** The app's bundled utility entry, which calls serveUtility(). Absolute. */
  entry: string;
  /** The cdylib. Absolute. A path inside app.asar is rewritten to app.asar.unpacked and must exist. */
  library: string;
  /** Structured-cloneable, sent to the page with its port and to the utility in its init message,
   *  e.g. () => ({ dataDir: app.getPath("userData") }). */
  info?: (wc: WebContents) => unknown;
  /** Starts each utility with --inspect=0. Ignored, with a warning, when app.isPackaged. */
  inspect?: boolean;
  /** Which frames may connect. Default: only the window's top frame. */
  allowFrame?: (frame: WebFrameMain) => boolean;
  onUtility?: (event: UtilityEvent) => void;
}

/** `expected` is false only for a failed start, or for a live utility that
 * exits non-zero before main ends its connection: a crash. */
export type UtilityEvent =
  | { kind: "spawn"; id: string; pid: number }
  | { kind: "ready"; id: string; pid: number }
  | {
      kind: "exit";
      id: string;
      pid: number | undefined;
      code: number;
      expected: boolean;
    };

/** Main's Electron objects, passed in so that tests can fake them. */
export interface MainDeps {
  ipcMain: Pick<IpcMain, "on">;
  utilityProcess: {
    fork(
      modulePath: string,
      args: string[],
      options: ForkOptions,
    ): UtilityProcess;
  };
  MessageChannelMain: new () => {
    port1: MessagePortMain;
    port2: MessagePortMain;
  };
  isPackaged(): boolean;
  fileExists(path: string): boolean;
  warn(message: string): void;
  /** For a utility that outlives its kill(); a pid already gone is ignored. */
  sigkill(pid: number): void;
}

export interface MainApi {
  serve(wc: WebContents, options: ServeOptions): { close(): void };
}

/** One served webContents. */
interface Served {
  wc: WebContents;
  options: ServeOptions;
  allowFrame(frame: WebFrameMain): boolean;
  context: ConnectionContext;
  /** The newest request, while the utility before it exits. */
  waiting: { id: string; frame: WebFrameMain } | undefined;
}

const NOT_SERVED =
  "the main process does not serve this window; call serve() from @ubjs/electron/main for it before it loads";
const NOT_ALLOWED =
  "the main process serves this window, but its allowFrame does not admit this frame";

function absolutePath(what: "entry" | "library", path: unknown): string {
  if (typeof path !== "string" || !isAbsolute(path)) {
    throw new TypeError(
      `serve() needs an absolute ${what} path, not ${JSON.stringify(path)}`,
    );
  }
  return path;
}

/** `serve` is a closure, so the entry can export the function itself. */
export function createMain(deps: MainDeps): MainApi {
  const registry = new Map<number, Served>();
  // A wc.id's newest utility, until it exits. Kept apart from the registry,
  // so a webContents served again after close() still has at most one.
  const running = new Map<number, UtilityConnection>();

  const fail = (
    wc: FrameOwner,
    frame: WebFrameMain,
    id: string,
    reason: FailureReason,
    message: string,
  ): void => sendFailure(wc, frame, deps.warn, id, reason, message);

  // A throw here would be an uncaught exception in main.
  const mayAsk = (served: Served, frame: WebFrameMain): boolean => {
    try {
      return served.allowFrame(frame);
    } catch (e) {
      deps.warn(`allowFrame threw, so the frame may not ask: ${messageOf(e)}`);
      return false;
    }
  };

  const start = (served: Served, id: string, frame: WebFrameMain): void => {
    const startFailed = (what: string, e: unknown) =>
      fail(served.wc, frame, id, "start-failed", `${what}: ${messageOf(e)}`);
    let info: unknown;
    try {
      info = served.options.info?.(served.wc);
    } catch (e) {
      startFailed("serve()'s info threw", e);
      return;
    }
    // Node's structured clone is close to Electron's serializer, not the same:
    // it catches most values Electron cannot send before anything forks.
    try {
      info = structuredClone(info);
    } catch (e) {
      startFailed("serve()'s info cannot be sent to the page", e);
      return;
    }
    let connection: UtilityConnection;
    try {
      connection = new UtilityConnection(served.context, id, frame, info);
    } catch (e) {
      startFailed("the utility did not start", e);
      return;
    }
    const wcId = served.wc.id;
    running.set(wcId, connection);
    connection.whenExited(() => {
      if (running.get(wcId) === connection) running.delete(wcId);
    });
  };

  const launch = (served: Served): void => {
    const next = served.waiting;
    served.waiting = undefined;
    if (next !== undefined) start(served, next.id, next.frame);
  };

  // One utility per webContents: the newest request retires the one before
  // it, and forks only once that has exited.
  const request = (served: Served, id: string, frame: WebFrameMain): void => {
    const superseded = served.waiting;
    served.waiting = { id, frame };
    if (superseded !== undefined) {
      // Already waiting on the old utility's exit.
      fail(
        served.wc,
        superseded.frame,
        superseded.id,
        "start-failed",
        REPLACED,
      );
      return;
    }
    const old = running.get(served.wc.id);
    if (old === undefined) {
      launch(served);
      return;
    }
    old.retire("replaced");
    old.whenExited(() => launch(served));
  };

  // One handler for every window, so an unserved window or a declined frame
  // hears not-served instead of waiting out its timeout.
  deps.ipcMain.on(CONNECT, (event: IpcMainEvent, payload: unknown) => {
    const frame = event.senderFrame;
    if (!frame) return;
    const id = (payload as { id?: unknown } | null | undefined)?.id;
    if (!isConnectionId(id)) return;
    const served = registry.get(event.sender.id);
    if (served === undefined) {
      fail(event.sender, frame, id, "not-served", NOT_SERVED);
      return;
    }
    if (!mayAsk(served, frame)) {
      fail(event.sender, frame, id, "not-served", NOT_ALLOWED);
      return;
    }
    request(served, id, frame);
  });

  const serve = (wc: WebContents, options: ServeOptions) => {
    // `in` finds the getter on the prototype without calling it.
    if (
      typeof wc !== "object" ||
      wc === null ||
      typeof wc.id !== "number" ||
      !("mainFrame" in wc)
    ) {
      throw new TypeError(
        "serve() needs a webContents, such as win.webContents, not a window",
      );
    }
    // Its destroyed event has fired, so nothing would remove it.
    if (wc.isDestroyed()) {
      throw new Error(
        `serve() was given webContents ${wc.id}, already destroyed`,
      );
    }
    const entry = absolutePath("entry", options.entry);
    const library = unpackedPath(absolutePath("library", options.library));
    if (!deps.fileExists(library)) {
      throw new Error(`serve() found no library at ${library}`);
    }
    if (registry.has(wc.id)) {
      throw new Error(
        `serve() already serves webContents ${wc.id}; close() it first`,
      );
    }
    const served: Served = {
      wc,
      options,
      // A subframe may be another origin.
      allowFrame: options.allowFrame ?? ((frame) => frame === wc.mainFrame),
      context: {
        wc,
        utilityProcess: deps.utilityProcess,
        MessageChannelMain: deps.MessageChannelMain,
        sigkill: deps.sigkill,
        warn: deps.warn,
        emit(event) {
          try {
            options.onUtility?.(event);
          } catch (e) {
            deps.warn(`onUtility threw: ${messageOf(e)}`);
          }
        },
        entry,
        library,
        // Inherited, so Rust's logging shows in main's terminal.
        forkOptions: {
          stdio: "inherit",
          serviceName: "@ubjs/electron",
          execArgv: inspectArgv(options.inspect, deps.isPackaged(), deps.warn),
        },
      },
      waiting: undefined,
    };
    registry.set(wc.id, served);

    // The renderer is gone, so nothing is sent to it; its reload asks again.
    const onGone = () => {
      served.waiting = undefined;
      running.get(wc.id)?.retire("frame-gone");
    };
    let serving = true;
    const stop = (cause: "closed" | "frame-gone") => {
      if (!serving) return;
      serving = false;
      wc.off("destroyed", onDestroyed);
      wc.off("render-process-gone", onGone);
      registry.delete(wc.id);
      const waiting = served.waiting;
      served.waiting = undefined;
      if (waiting !== undefined && cause === "closed") {
        fail(wc, waiting.frame, waiting.id, "start-failed", CLOSED);
      }
      running.get(wc.id)?.retire(cause);
    };
    const onDestroyed = () => stop("frame-gone");
    // No navigation listener: a reload or a navigation retires nothing; the
    // next request does.
    wc.once("destroyed", onDestroyed);
    wc.on("render-process-gone", onGone);
    return { close: () => stop("closed") };
  };

  return { serve };
}

const SHARED = Symbol.for("@ubjs/electron/main");

/** One MainApi per process, kept on globalThis[Symbol.for("@ubjs/electron/main")].
 * An app can load both the ESM and the CJS build, and they must share one
 * handler and one registry. */
export function sharedMain(create: () => MainApi): MainApi {
  const store = globalThis as Record<symbol, MainApi | undefined>;
  return (store[SHARED] ??= create());
}
