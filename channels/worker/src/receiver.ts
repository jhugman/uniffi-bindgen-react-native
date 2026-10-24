/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  UniffiInternalError,
  type ModuleDefinitions,
  type UniffiRustCallStatus,
} from "@ubjs/core";
import type { ChannelListener, ChannelPort } from "./port.js";
import {
  fromWireError,
  isChannelMessage,
  toWireError,
  type ChannelMessage,
  type CallbackReturnMessage,
} from "./messages.js";
import {
  compilePlan,
  type CallbackLifetime,
  type CallbackPlan,
  type FunctionPlan,
  type ModulePlan,
  type ValuePlan,
} from "./plan.js";
import { CallbackRegistry, ForwarderCache } from "./registry.js";
import {
  marshalIn,
  marshalOut,
  wholeBuffer,
  type MarshalContext,
} from "./marshal.js";
import { tagHandle, untagHandle } from "./handles.js";
import type { Receiver, RegisteredPlayer } from "./types.js";

export interface ReceiverOptions {
  portId?: number;
}

type Settle = (msg: CallbackReturnMessage) => void;

interface DroppedClient {
  handle: bigint;
  free: (h: bigint) => void;
}

export class ReceiverCore {
  private registry = new CallbackRegistry();
  private forwarders = new ForwarderCache();
  private pendingCallbacks = new Map<number, Settle>();
  // The signal handed to each async entry still running, by call id.
  private inflight = new Map<number, AbortController>();
  private nextId = 1;
  private nextLocalHandle = 1n;
  private dropped = new Map<bigint, DroppedClient>();
  private freeRequested = new Set<bigint>();
  readonly refcounts = new Map<string, Map<bigint, number>>();
  private returnCtx: MarshalContext;
  private callbackCtx: MarshalContext;
  private listener: ChannelListener;
  private closed = false;

  // One function object for every async method's dropped-struct: wasm2
  // installs a permanent table slot per distinct closure.
  private localFree = (h: bigint): void => {
    const client = this.dropped.get(h);
    if (client) {
      this.dropped.delete(h);
      client.free(client.handle);
    } else {
      this.freeRequested.add(h);
    }
  };

  constructor(
    private plan: ModulePlan,
    private player: RegisteredPlayer,
    private port: ChannelPort,
    private portId: number,
  ) {
    const shared = {
      registry: this.registry,
      forwarders: this.forwarders,
      callbacks: plan.callbacks,
      makeForwarder: (id: number, p: CallbackPlan, l: CallbackLifetime) =>
        this.makeForwarder(id, p, l),
      handleIn: (h: bigint) => tagHandle(h, portId),
      handleOut: (h: bigint) => untagHandle(h, portId),
    };
    // Views the player returns from a synchronous call alias wasm memory and
    // are ours to free; an async entry's result and the views the player
    // passes into a callback are JS-owned copies already.
    this.returnCtx = {
      ...shared,
      bufferOut: (v) => {
        const copy = new Uint8Array(v.byteLength);
        copy.set(v);
        player.rustbuffer_free(v);
        return copy;
      },
    };
    this.callbackCtx = { ...shared, bufferOut: wholeBuffer };
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

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.port.removeEventListener("message", this.listener);
    this.pendingCallbacks.clear();
    // No reply can be read once closed, so cancel each entry still running:
    // its driver then completes, frees the future and settles unposted.
    for (const controller of this.inflight.values()) controller.abort();
    this.inflight.clear();
    this.port.close?.();
  }

  private onMessage(data: unknown): void {
    if (!isChannelMessage(data)) {
      console.warn("worker: receiver dropped a malformed message", data);
      return;
    }
    switch (data.kind) {
      case "call":
        return this.onCall(data);
      case "callback":
        return void this.onCallback(data);
      case "callback-return": {
        const settle = this.pendingCallbacks.get(data.id);
        if (!settle) return;
        this.pendingCallbacks.delete(data.id);
        return settle(data);
      }
      case "release":
        return this.registry.release(data.cb);
      case "cancel":
        // A call that already settled has nothing left to abort.
        return this.inflight.get(data.id)?.abort();
      default:
        return;
    }
  }

  private onCall(msg: Extract<ChannelMessage, { kind: "call" }>): void {
    const id = msg.id;
    const plan = this.plan.functions.get(msg.fn);
    const isAsync = plan?.isAsync === true;
    try {
      if (!plan) throw new Error(`worker: unknown function "${msg.fn}"`);
      const args = plan.args.map((p, i) => this.inboundArg(p, msg.args[i]));
      let status: UniffiRustCallStatus | undefined;
      if (plan.hasRustCallStatus) {
        status = { code: 0 };
        args.push(status);
      }
      if (isAsync) {
        const controller = new AbortController();
        this.inflight.set(id, controller);
        args.push(controller.signal);
      }
      const raw: any = this.player[msg.fn](...args);
      // Wait only on a thenable: a non-thenable result must reach postMessage synchronously so a sync port can answer.
      if (raw && typeof raw.then === "function") {
        raw.then(
          (v: unknown) =>
            this.settle(id, isAsync, () =>
              this.postReturn(id, plan, v, status),
            ),
          (e: unknown) => this.settle(id, isAsync, () => this.postError(id, e)),
        );
        return;
      }
      // A player that predates async entries starts the future and returns
      // its handle, which must not be posted as the value.
      if (isAsync) throw new UniffiInternalError.IncompatiblePlayer();
      this.settle(id, isAsync, () => this.postReturn(id, plan, raw, status));
    } catch (e) {
      this.settle(id, isAsync, () => this.postError(id, e));
    }
  }

  // Where a call's reply is posted: an async call leaves `inflight`, nothing
  // is posted after close, and a reply that cannot be marshalled or posted
  // goes back as an error instead.
  private settle(id: number, isAsync: boolean, post: () => void): void {
    if (isAsync) this.inflight.delete(id);
    if (this.closed) return;
    try {
      post();
    } catch (e) {
      this.postError(id, e);
    }
  }

  // An async entry writes the status as it settles, so it is read only here.
  private postReturn(
    id: number,
    plan: FunctionPlan,
    raw: unknown,
    status: UniffiRustCallStatus | undefined,
  ): void {
    const transfer: Transferable[] = [];
    // An async entry's RustBuffer result is already a JS-owned copy of exact
    // length, so it is transferred as it is.
    const ctx = plan.isAsync ? this.callbackCtx : this.returnCtx;
    const value =
      status && status.code !== 0
        ? undefined
        : marshalOut(plan.ret, raw, ctx, transfer);
    this.port.postMessage(
      { kind: "return", id, ok: true, value, status },
      transfer,
    );
  }

  private postError(id: number, e: unknown): void {
    this.port.postMessage({
      kind: "return",
      id,
      ok: false,
      error: toWireError(e),
    });
  }

  // Functions this receiver registered in its own CallbackRegistry (e.g. an
  // async vtable method's completion callback) are invoked by the client
  // posting one of these back to us; mirrors SenderCore.onCallback.
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
        marshalIn(p, msg.args[i], this.callbackCtx),
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
                  this.callbackCtx,
                  transfer,
                ),
              }
            : { code: result.code, errorBuf: result.errorBuf };
      } else {
        value = marshalOut(entry.plan.ret, result, this.callbackCtx, transfer);
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

  private inboundArg(plan: ValuePlan, wire: unknown): unknown {
    const value = marshalIn(plan, wire, this.callbackCtx);
    if (plan.kind === "struct")
      this.refcountVtable(plan.name, value as Record<string, unknown>);
    return value;
  }

  // Rust calls clone/free synchronously and expects the same handle back;
  // only the last free needs to reach the client. Keyed by interface name
  // because every callback interface has its own UniffiHandleMap, so two
  // interfaces routinely reuse the same numeric handle.
  private refcountVtable(name: string, obj: Record<string, unknown>): void {
    const forwardFree = obj.uniffi_free;
    if (
      typeof forwardFree !== "function" ||
      typeof obj.uniffi_clone !== "function"
    )
      return;
    const refcounts = this.refcounts;
    obj.uniffi_clone = (h: bigint) => {
      let counts = refcounts.get(name);
      if (!counts) refcounts.set(name, (counts = new Map()));
      counts.set(h, (counts.get(h) ?? 1) + 1);
      return h;
    };
    obj.uniffi_free = (h: bigint) => {
      const counts = refcounts.get(name);
      const n = (counts?.get(h) ?? 1) - 1;
      if (counts && n > 0) {
        counts.set(h, n);
        return;
      }
      if (counts) {
        counts.delete(h);
        if (counts.size === 0) refcounts.delete(name);
      }
      (forwardFree as (h: bigint) => void)(h);
    };
  }

  private postCallback(
    id: number,
    plan: CallbackPlan,
    args: unknown[],
    settle: Settle,
  ): void {
    const transfer: Transferable[] = [];
    const wire = plan.args.map((p, i) =>
      marshalOut(p, args[i], this.callbackCtx, transfer),
    );
    const reqId = this.nextId++;
    this.pendingCallbacks.set(reqId, settle);
    this.port.postMessage(
      { kind: "callback", id: reqId, cb: id, args: wire },
      transfer,
    );
  }

  private releaseIfInvocation(id: number, lifetime: CallbackLifetime): void {
    if (lifetime !== "invocation") return;
    this.forwarders.release(id);
    this.port.postMessage({ kind: "release", cb: id });
  }

  private makeForwarder(
    id: number,
    plan: CallbackPlan,
    lifetime: CallbackLifetime,
  ): Function {
    const logFailure: Settle = (m) => {
      if (!m.ok)
        console.error(
          `worker: callback "${plan.name}" failed on the client: ${m.error.name}: ${m.error.message}`,
        );
    };

    if (plan.retTag === "Void" && !plan.hasRustCallStatus) {
      return (...args: unknown[]) => {
        this.postCallback(id, plan, args, logFailure);
        this.releaseIfInvocation(id, lifetime);
      };
    }

    if (plan.outReturn && plan.retTag === "Struct" && !plan.hasRustCallStatus) {
      return (...args: unknown[]) => {
        const local = this.nextLocalHandle++;
        this.postCallback(id, plan, args, (m) => {
          logFailure(m);
          if (!m.ok) return;
          const client = marshalIn(
            plan.ret,
            m.value,
            this.callbackCtx,
          ) as DroppedClient;
          this.dropped.set(local, client);
          if (this.freeRequested.delete(local)) this.localFree(local);
        });
        this.releaseIfInvocation(id, lifetime);
        return { handle: local, free: this.localFree };
      };
    }

    // Anything that must hand Rust a value now: only a synchronous port can.
    return (...args: unknown[]) => {
      let reply: CallbackReturnMessage | undefined;
      this.postCallback(id, plan, args, (m) => {
        reply = m;
      });
      this.releaseIfInvocation(id, lifetime);
      if (!reply) {
        throw new Error(
          `worker: callback "${plan.name}" needs a synchronous reply; ` +
            "synchronous callback-interface methods are not supported over an asynchronous channel, " +
            "enable forceAsync for this interface",
        );
      }
      if (!reply.ok) throw fromWireError(reply.error);
      if (plan.hasRustCallStatus) {
        const v = reply.value as {
          pointee?: unknown;
          code?: number;
          errorBuf?: Uint8Array;
        };
        return "pointee" in v
          ? { pointee: marshalIn(plan.ret, v.pointee, this.callbackCtx) }
          : v;
      }
      return marshalIn(plan.ret, reply.value, this.callbackCtx);
    };
  }
}

export function createReceiver(
  defs: ModuleDefinitions,
  player: RegisteredPlayer,
  port: ChannelPort,
  opts: ReceiverOptions = {},
): Receiver {
  const core = new ReceiverCore(
    compilePlan(defs),
    player,
    port,
    opts.portId ?? 0,
  );
  return { close: () => core.close() };
}
