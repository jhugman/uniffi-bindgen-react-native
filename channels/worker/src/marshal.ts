/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { CallbackLifetime, CallbackPlan, ValuePlan } from "./plan.js";
import type { CallbackRegistry, ForwarderCache } from "./registry.js";

export interface MarshalContext {
  registry: CallbackRegistry;
  forwarders: ForwarderCache;
  callbacks: Map<string, CallbackPlan>;
  makeForwarder(
    id: number,
    plan: CallbackPlan,
    lifetime: CallbackLifetime,
  ): Function;
  handleIn(h: bigint): bigint;
  handleOut(h: bigint): bigint;
  bufferOut(v: Uint8Array): Uint8Array;
}

/** A view that owns its whole ArrayBuffer can be transferred; any other
 * view is copied into one that can. */
export function wholeBuffer(v: Uint8Array): Uint8Array {
  if (v.byteOffset === 0 && v.byteLength === v.buffer.byteLength) return v;
  return v.slice();
}

export function marshalOut(
  plan: ValuePlan,
  value: unknown,
  ctx: MarshalContext,
  transfer: Transferable[],
): unknown {
  switch (plan.kind) {
    case "pass":
      return value;
    case "handle":
      return ctx.handleOut(value as bigint);
    case "buffer": {
      if (!(value instanceof Uint8Array)) {
        throw new Error(`worker: expected a Uint8Array, got ${typeof value}`);
      }
      const w = ctx.bufferOut(value);
      if (!transfer.includes(w.buffer)) transfer.push(w.buffer as ArrayBuffer);
      return w;
    }
    case "callback": {
      if (typeof value !== "function") {
        throw new Error(
          `worker: expected a function for callback "${plan.name}", got ${typeof value}`,
        );
      }
      return ctx.registry.register(value, ctx.callbacks.get(plan.name)!);
    }
    case "struct": {
      const src = value as Record<string, unknown>;
      const out: Record<string, unknown> = {};
      for (const f of plan.fields)
        out[f.name] = marshalOut(f.plan, src[f.name], ctx, transfer);
      return out;
    }
  }
}

export function marshalIn(
  plan: ValuePlan,
  value: unknown,
  ctx: MarshalContext,
): unknown {
  switch (plan.kind) {
    case "pass":
      return value;
    case "handle":
      return ctx.handleIn(value as bigint);
    case "buffer":
      return value;
    case "callback": {
      if (typeof value !== "number") {
        throw new Error(
          `worker: expected a callback id for "${plan.name}", got ${typeof value}`,
        );
      }
      const cbPlan = ctx.callbacks.get(plan.name)!;
      return ctx.forwarders.forwarderFor(value, () =>
        ctx.makeForwarder(value, cbPlan, plan.lifetime),
      );
    }
    case "struct": {
      const src = value as Record<string, unknown>;
      const out: Record<string, unknown> = {};
      for (const f of plan.fields)
        out[f.name] = marshalIn(f.plan, src[f.name], ctx);
      return out;
    }
  }
}
