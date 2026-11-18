/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { EventEmitter } from "node:events";
import type { ForkOptions } from "electron";
import type { MainDeps } from "../../src/main/core.js";
import { CONNECT, ERROR, READY } from "../../src/protocol.js";
import {
  checkPorts,
  fakeMessageChannelMain,
  type FakePortMain,
} from "./ports.js";

// What Electron throws for a frame that a navigation or a close has disposed.
const DISPOSED =
  "Render frame was disposed before WebFrameMain could be accessed";

/** A WebFrameMain. While it is destroyed or `failSends` is set, postMessage
 * throws, so a post that main does not guard fails the test, and send() logs
 * and drops, as Electron's does. */
export class FakeFrame {
  posted: { channel: string; message: unknown; ports: unknown[] }[] = [];
  sent: { channel: string; args: unknown[] }[] = [];
  /** What Electron's send() would have logged instead of throwing. */
  consoleErrors: string[] = [];
  destroyed = false;
  detached = false;
  failSends = false;

  constructor(public url = "file:///app/index.html") {}

  isDestroyed(): boolean {
    return this.destroyed;
  }

  postMessage(channel: string, message: unknown, ports: unknown[] = []): void {
    this.check();
    checkPorts(ports);
    this.posted.push({
      channel,
      message: structuredClone(message),
      ports: [...ports],
    });
  }

  send(channel: string, ...args: unknown[]): void {
    if (this.destroyed || this.failSends) {
      this.consoleErrors.push(`Error sending from webFrameMain: ${DISPOSED}`);
      return;
    }
    this.sent.push({ channel, args: structuredClone(args) });
  }

  private check(): void {
    if (this.destroyed || this.failSends) throw new Error(DISPOSED);
  }
}

export class FakeWebContents extends EventEmitter {
  mainFrame = new FakeFrame();
  destroyed = false;

  constructor(readonly id: number) {
    super();
  }

  isDestroyed(): boolean {
    return this.destroyed;
  }
}

/** A UtilityProcess. Its helpers play the utility's side. */
export class FakeUtility extends EventEmitter {
  pid: number | undefined;
  posted: { message: unknown; ports: unknown[] }[] = [];
  /** kill() calls, whether or not they did anything. */
  killed = 0;
  /** Thrown by postMessage, as Electron's serializer throws for what it cannot send. */
  postError: Error | undefined;

  postMessage(message: unknown, transfer: readonly unknown[] = []): void {
    if (this.postError) throw this.postError;
    checkPorts(transfer);
    this.posted.push({
      message: structuredClone(message),
      ports: [...transfer],
    });
  }

  // Electron's does nothing, and returns false, until the child has spawned.
  kill(): boolean {
    this.killed += 1;
    return this.pid !== undefined;
  }

  spawn(pid: number): void {
    this.pid = pid;
    this.emit("spawn");
  }

  ready(): void {
    this.emit("message", { type: READY });
  }

  fail(message: string): void {
    this.emit("message", {
      type: ERROR,
      message,
      stack: `Error: ${message}\n    at register`,
    });
  }

  exit(code: number): void {
    this.pid = undefined;
    this.emit("exit", code);
  }
}

/** Main's Electron objects, recording every fork, channel, warning and SIGKILL. */
export function fakeMain() {
  const ipcMain = new EventEmitter();
  const forks: {
    entry: string;
    args: string[];
    options: ForkOptions;
    child: FakeUtility;
  }[] = [];
  const channels: { port1: FakePortMain; port2: FakePortMain }[] = [];
  const warnings: string[] = [];
  const sigkills: number[] = [];
  const files = new Set(["/app/lib.dylib"]);
  let packaged = false;
  let forkError: Error | undefined;
  let postError: Error | undefined;

  class MessageChannelMain {
    port1: FakePortMain;
    port2: FakePortMain;
    constructor() {
      const channel = fakeMessageChannelMain();
      this.port1 = channel.port1;
      this.port2 = channel.port2;
      channels.push(channel);
    }
  }

  const deps = {
    ipcMain,
    utilityProcess: {
      fork(entry: string, args: string[], options: ForkOptions) {
        if (forkError) throw forkError;
        const child = new FakeUtility();
        child.postError = postError;
        forks.push({ entry, args, options, child });
        return child;
      },
    },
    MessageChannelMain,
    isPackaged: () => packaged,
    fileExists: (path: string) => files.has(path),
    warn: (message: string) => void warnings.push(message),
    sigkill: (pid: number) => void sigkills.push(pid),
  } as unknown as MainDeps;

  return {
    deps,
    ipcMain,
    forks,
    channels,
    warnings,
    sigkills,
    files,
    setPackaged(value: boolean) {
      packaged = value;
    },
    /** Every later fork throws `error`. */
    failFork(error: Error) {
      forkError = error;
    },
    /** Every later fork's child throws `error` from postMessage. */
    failInit(error: Error) {
      postError = error;
    },
    /** A page's connect request, as ipcMain delivers it. */
    request(wc: FakeWebContents, frame: FakeFrame | null, payload: unknown) {
      ipcMain.emit(CONNECT, { sender: wc, senderFrame: frame }, payload);
    },
  };
}
