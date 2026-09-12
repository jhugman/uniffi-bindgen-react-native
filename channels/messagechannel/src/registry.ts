/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { CallbackPlan } from "./plan.js";

export interface Registered {
  fn: Function;
  plan: CallbackPlan;
}

/** Functions this end owns, keyed by the id the other end sees. */
export class CallbackRegistry {
  private ids = new WeakMap<Function, number>();
  private entries = new Map<number, Registered>();
  private next = 1;

  register(fn: Function, plan: CallbackPlan): number {
    const existing = this.ids.get(fn);
    if (existing !== undefined && this.entries.has(existing)) return existing;
    const id = this.next++;
    this.ids.set(fn, id);
    this.entries.set(id, { fn, plan });
    return id;
  }

  get(id: number): Registered | undefined {
    return this.entries.get(id);
  }

  release(id: number): void {
    const entry = this.entries.get(id);
    if (!entry) return;
    this.entries.delete(id);
    this.ids.delete(entry.fn);
  }

  get size(): number {
    return this.entries.size;
  }
}

/** Forwarding functions for ids the other end owns. One function object per
 * id: wasm2 charges a permanent function-table slot per distinct closure. */
export class ForwarderCache {
  private fns = new Map<number, Function>();

  forwarderFor(id: number, build: () => Function): Function {
    let fn = this.fns.get(id);
    if (!fn) {
      fn = build();
      this.fns.set(id, fn);
    }
    return fn;
  }

  release(id: number): void {
    this.fns.delete(id);
  }

  get size(): number {
    return this.fns.size;
  }
}
