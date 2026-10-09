/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ModuleDefinitions } from "@ubjs/core";
import type { ChannelListener, ChannelPort } from "./port.js";
import {
  fromWireError,
  isChannelMessage,
  toWireError,
  type ChannelMessage,
} from "./messages.js";
import {
  compilePlan,
  type CallbackLifetime,
  type CallbackPlan,
  type FunctionPlan,
  type ModulePlan,
} from "./plan.js";
import { CallbackRegistry, ForwarderCache } from "./registry.js";
import {
  marshalIn,
  marshalOut,
  wholeBuffer,
  type MarshalContext,
} from "./marshal.js";
import { ChannelClosedError, type Sender } from "./types.js";

interface Settle {
  resolve(value: unknown): void;
  reject(error: Error): void;
}

interface PendingCall extends Settle {
  plan: FunctionPlan;
  status: { code: number; errorBuf?: Uint8Array } | undefined;
}

export class SenderCore {
  private pending = new Map<number, PendingCall>();
  private nextId = 1;
  private registry = new CallbackRegistry();
  private forwarders = new ForwarderCache();
  private ctx: MarshalContext;
  private listener: ChannelListener;
  private closed = false;

  constructor(
    private plan: ModulePlan,
    private port: ChannelPort,
  ) {
    this.ctx = {
      registry: this.registry,
      forwarders: this.forwarders,
      callbacks: plan.callbacks,
      makeForwarder: (id, cbPlan, lifetime) =>
        this.makeForwarder(id, cbPlan, lifetime),
      handleIn: (h) => h,
      handleOut: (h) => h,
      bufferOut: wholeBuffer,
    };
    this.listener = (ev) => this.onMessage(ev.data);
    port.addEventListener("message", this.listener);
    port.start?.();
  }

  get registrySize(): number {
    return this.registry.size;
  }
  get forwarderCount(): number {
    return this.forwarders.size;
  }

  invoke(fn: string, args: unknown[]): Promise<unknown> {
    const plan = this.lookup(fn);
    return new Promise((resolve, reject) =>
      this.dispatch(plan, args, { resolve, reject }),
    );
  }

  invokeSync(fn: string, args: unknown[]): unknown {
    const plan = this.lookup(fn);
    let done = false,
      value: unknown,
      error: Error | undefined;
    this.dispatch(plan, args, {
      resolve: (v) => {
        done = true;
        value = v;
      },
      reject: (e) => {
        done = true;
        error = e;
      },
    });
    if (!done)
      throw new Error(
        `worker: "${fn}" did not return synchronously; the port is not synchronous`,
      );
    if (error) throw error;
    return value;
  }

  rustbuffer_alloc(n: number): Uint8Array {
    return new Uint8Array(n);
  }
  rustbuffer_free(_view: Uint8Array): void {
    /* transferred views are already detached */
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.port.removeEventListener("message", this.listener);
    for (const p of this.pending.values()) p.reject(new ChannelClosedError());
    this.pending.clear();
    this.port.close?.();
  }

  private lookup(fn: string): FunctionPlan {
    const plan = this.plan.functions.get(fn);
    if (!plan) throw new Error(`worker: unknown function "${fn}"`);
    return plan;
  }

  private dispatch(plan: FunctionPlan, args: unknown[], settle: Settle): void {
    if (this.closed) return settle.reject(new ChannelClosedError());
    let status: PendingCall["status"];
    let userArgs = args;
    if (plan.hasRustCallStatus) {
      status = args[args.length - 1] as PendingCall["status"];
      userArgs = args.slice(0, -1);
    }
    const transfer: Transferable[] = [];
    const wire = plan.args.map((p, i) =>
      marshalOut(p, userArgs[i], this.ctx, transfer),
    );
    const id = this.nextId++;
    this.pending.set(id, { ...settle, plan, status });
    this.port.postMessage(
      { kind: "call", id, fn: plan.name, args: wire },
      transfer,
    );
  }

  private onMessage(data: unknown): void {
    if (!isChannelMessage(data)) {
      console.warn("worker: sender dropped a malformed message", data);
      return;
    }
    switch (data.kind) {
      case "return":
        return this.onReturn(data);
      case "callback":
        return void this.onCallback(data);
      case "callback-return":
        return this.onCallbackReturn(data);
      case "release":
        return this.registry.release(data.cb);
      default:
        return; // "call" is not addressed to a sender
    }
  }

  private onReturn(msg: Extract<ChannelMessage, { kind: "return" }>): void {
    const p = this.pending.get(msg.id);
    if (!p) return; // normal after close()
    this.pending.delete(msg.id);
    if (!msg.ok) return p.reject(fromWireError(msg.error));
    if (p.status && msg.status) {
      p.status.code = msg.status.code;
      if (msg.status.errorBuf) p.status.errorBuf = msg.status.errorBuf;
      if (msg.status.code !== 0) return p.resolve(undefined);
    }
    p.resolve(marshalIn(p.plan.ret, msg.value, this.ctx));
  }

  private async onCallback(
    msg: Extract<ChannelMessage, { kind: "callback" }>,
  ): Promise<void> {
    const entry = this.registry.get(msg.cb);
    if (!entry) {
      this.port.postMessage({
        kind: "callback-return",
        id: msg.id,
        ok: false,
        error: { name: "Error", message: `unknown callback id ${msg.cb}` },
      });
      return;
    }
    const transfer: Transferable[] = [];
    try {
      const args = entry.plan.args.map((p, i) =>
        marshalIn(p, msg.args[i], this.ctx),
      );
      // Await only a thenable: a non-thenable result must reach postMessage synchronously so a sync port can answer a sync vtable method.
      let result: any = entry.fn(...args);
      if (result && typeof result.then === "function") result = await result;
      let value: unknown;
      if (entry.plan.hasRustCallStatus) {
        value =
          "pointee" in result
            ? {
                pointee: marshalOut(
                  entry.plan.ret,
                  result.pointee,
                  this.ctx,
                  transfer,
                ),
              }
            : { code: result.code, errorBuf: result.errorBuf };
      } else {
        value = marshalOut(entry.plan.ret, result, this.ctx, transfer);
      }
      this.port.postMessage(
        { kind: "callback-return", id: msg.id, ok: true, value },
        transfer,
      );
    } catch (e) {
      this.port.postMessage({
        kind: "callback-return",
        id: msg.id,
        ok: false,
        error: toWireError(e),
      });
    }
  }

  // Replies to callbacks this end forwarded (see makeForwarder): nothing
  // waits on them, so only a failure is worth surfacing.
  private onCallbackReturn(
    msg: Extract<ChannelMessage, { kind: "callback-return" }>,
  ): void {
    if (!msg.ok)
      console.error(
        `worker: forwarded callback ${msg.id} failed: ${msg.error.name}: ${msg.error.message}`,
      );
  }

  private makeForwarder(
    id: number,
    cbPlan: CallbackPlan,
    lifetime: CallbackLifetime,
  ): Function {
    if (cbPlan.retTag !== "Void") {
      return () => {
        throw new Error(
          `worker: callback "${cbPlan.name}" returning ${cbPlan.retTag} cannot be forwarded from the sender`,
        );
      };
    }
    return (...args: unknown[]) => {
      const transfer: Transferable[] = [];
      const wire = cbPlan.args.map((p, i) =>
        marshalOut(p, args[i], this.ctx, transfer),
      );
      this.port.postMessage(
        { kind: "callback", id: this.nextId++, cb: id, args: wire },
        transfer,
      );
      if (lifetime === "invocation") {
        this.forwarders.release(id);
        this.port.postMessage({ kind: "release", cb: id });
      }
    };
  }
}

export function createSender<D extends ModuleDefinitions>(
  defs: D,
  port: ChannelPort,
): Sender<D> {
  const core = new SenderCore(compilePlan(defs), port);
  const sender: Record<string, unknown> = Object.create(null);
  for (const name of Object.keys(defs.functions)) {
    sender[name] = (...args: unknown[]) => core.invoke(name, args);
  }
  sender.close = () => core.close();
  sender.rustbuffer_alloc = (n: number) => core.rustbuffer_alloc(n);
  sender.rustbuffer_free = (v: Uint8Array) => core.rustbuffer_free(v);
  return sender as unknown as Sender<D>;
}
