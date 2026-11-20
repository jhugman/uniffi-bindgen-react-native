/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type {
  ForkOptions,
  MessagePortMain,
  UtilityProcess,
  WebFrameMain,
} from "electron";
import {
  DISCONNECTED,
  ERROR,
  INIT,
  PORT,
  READY,
  isOfType,
  type DisconnectedNotice,
  type ErrorMessage,
  type FailureReason,
  type InitMessage,
  type PortReply,
  type ReadyMessage,
} from "../protocol.js";
import type { MainDeps, UtilityEvent } from "./core.js";
import {
  messageOf,
  sendFailure,
  sendToFrame,
  type FrameOwner,
} from "./frame.js";

/** What every utility forked for one served webContents shares. */
export interface ConnectionContext {
  /** The served webContents, whose frames the connections reply to. */
  wc: FrameOwner;
  utilityProcess: MainDeps["utilityProcess"];
  MessageChannelMain: MainDeps["MessageChannelMain"];
  warn(message: string): void;
  /** Hands an event to the app's onUtility. */
  emit(event: UtilityEvent): void;
  entry: string;
  library: string;
  forkOptions: ForkOptions;
}

export type ConnectionState = "starting" | "live" | "retired";

/** Why main ended a connection. A live utility that exits on its own has none. */
export type RetireCause = "replaced" | "closed" | "frame-gone" | "start-failed";

export const REPLACED = "replaced by a newer connect() in this window";
export const CLOSED = "serve() was closed for this window";

// What a page still waiting for its port hears when main ends its connection.
const RETIRED_WHILE_STARTING: Partial<Record<RetireCause, string>> = {
  replaced: REPLACED,
  closed: CLOSED,
};

/** Electron's kill() does nothing before spawn, so a child not yet spawned is
 * killed when it spawns. Once sent, Electron escalates to SIGKILL by itself
 * if the child has not exited within 2 s. */
function killChild(child: UtilityProcess): void {
  if (!child.kill()) child.once("spawn", () => child.kill());
}

/** One utility process, serving the page connection named by `id`. It tells
 * its own frame, once and by its own id, how the connection ended. */
export class UtilityConnection {
  private state: ConnectionState = "starting";
  private readonly child: UtilityProcess;
  /** Main's end, until the page is sent it. */
  private port2: MessagePortMain | undefined;
  /** The page holds its port, so it is owed a DISCONNECTED when the utility exits. */
  private portSent = false;
  private cause: RetireCause | undefined;
  private pid: number | undefined;
  private exited = false;
  private readonly onExited: (() => void)[] = [];

  /** Forks the utility and posts its init. A fork that throws throws here,
   * before any port exists. A throw after the fork fails the start like any
   * other, so the child is retired and its exit reported. */
  constructor(
    private readonly context: ConnectionContext,
    readonly id: string,
    readonly frame: WebFrameMain,
    private readonly info: unknown,
  ) {
    this.child = context.utilityProcess.fork(
      context.entry,
      [],
      context.forkOptions,
    );
    // An 'error' with no listener throws in main. Electron emits 'exit' after it.
    this.child.on("error", (type: string, location: string) =>
      context.warn(`the utility hit a ${type} at ${location}`),
    );
    this.child.on("spawn", () => this.onSpawn());
    this.child.on("message", (message: unknown) => this.onMessage(message));
    this.child.on("exit", (code: number) => this.onExit(code));

    let channel: { port1: MessagePortMain; port2: MessagePortMain } | undefined;
    try {
      channel = new context.MessageChannelMain();
      const init: InitMessage = {
        type: INIT,
        id,
        library: context.library,
        info,
      };
      // Electron holds a message posted before spawn, so the utility can start
      // opening the library as soon as it runs.
      this.child.postMessage(init, [channel.port1]);
    } catch (e) {
      channel?.port1.close();
      channel?.port2.close();
      this.failStart(
        "start-failed",
        `the utility did not start: ${messageOf(e)}`,
      );
      return;
    }
    this.port2 = channel.port2;
  }

  /** Runs `fn` once the utility has exited: now, if it already has. */
  whenExited(fn: () => void): void {
    if (this.exited) fn();
    else this.onExited.push(fn);
  }

  /** Ends the connection: kills the utility. For replaced or closed, a page still waiting
   * for its port is told now, and a page that holds its port is told when the
   * utility exits. A gone frame is told nothing; failStart has already told
   * the page of a failed start. */
  retire(cause: RetireCause): void {
    if (this.state === "retired") return;
    const wasStarting = this.state === "starting";
    this.state = "retired";
    this.cause = cause;
    this.port2?.close();
    this.port2 = undefined;
    const message = RETIRED_WHILE_STARTING[cause];
    if (wasStarting && message !== undefined) {
      this.reply("start-failed", message);
    }
    if (!this.exited) killChild(this.child);
  }

  private reply(reason: FailureReason, message: string): void {
    const { wc, warn } = this.context;
    sendFailure(wc, this.frame, warn, this.id, reason, message);
  }

  /** A startup failure rejects the page's connect(), sent or not, and retires. */
  private failStart(reason: FailureReason, message: string): void {
    if (this.state !== "starting") return;
    this.reply(reason, message);
    this.retire("start-failed");
  }

  private onSpawn(): void {
    // Electron sets pid before it emits spawn, and clears it at exit.
    this.pid = this.child.pid!;
    this.context.emit({ kind: "spawn", id: this.id, pid: this.pid });
  }

  private onMessage(message: unknown): void {
    if (isOfType<ReadyMessage>(message, READY)) this.onReady();
    else if (isOfType<ErrorMessage>(message, ERROR)) {
      // The utility prints nothing itself, so this is where its trace shows.
      if (this.state === "starting") {
        this.context.warn(
          `the utility failed to start: ${String(message.stack ?? message.message)}`,
        );
      }
      this.failStart("start-failed", String(message.message));
    }
  }

  private onReady(): void {
    if (this.state !== "starting") return;
    // Only now: a library that fails to open still rejects connect(), so the
    // app can fall back before it builds anything on the port.
    const port2 = this.port2!;
    const reply: PortReply = { id: this.id, info: this.info };
    const sent = sendToFrame(
      this.context.wc,
      this.frame,
      this.context.warn,
      (frame) => frame.postMessage(PORT, reply, [port2]),
    );
    if (!sent) {
      this.retire("frame-gone");
      return;
    }
    this.state = "live";
    this.portSent = true;
    this.port2 = undefined;
    this.context.emit({ kind: "ready", id: this.id, pid: this.child.pid! });
  }

  private onExit(code: number): void {
    this.exited = true;
    this.failStart(
      "start-failed",
      `the utility exited with code ${code} before it was ready`,
    );
    // A live utility exits 0 once its page's port closes.
    const expected =
      this.cause === undefined ? code === 0 : this.cause !== "start-failed";
    this.state = "retired";
    if (this.portSent && this.cause !== "frame-gone") {
      const notice: DisconnectedNotice = {
        id: this.id,
        code,
        reason: "utility-exited",
      };
      sendToFrame(this.context.wc, this.frame, this.context.warn, (frame) =>
        frame.send(DISCONNECTED, notice),
      );
    }
    this.context.emit({
      kind: "exit",
      id: this.id,
      pid: this.pid,
      code,
      expected,
    });
    for (const fn of this.onExited.splice(0)) fn();
  }
}
