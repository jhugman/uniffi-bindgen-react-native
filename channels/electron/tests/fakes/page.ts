/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { PageWindow } from "../../src/page/core.js";
import type { PROTOCOL } from "../../src/protocol.js";

type Listener = (ev: MessageEvent) => void;

/** The page's window, with the preload's bridge at `protocol` or none at all.
 * `dispatch` plays a window message, from this window unless told otherwise. */
export function fakeWindow(bridge: "none" | 1 | 2 = 1) {
  const listeners: Listener[] = [];
  const ids: string[] = [];
  /** How many listeners the window had at each ask. */
  const listenersAtAsk: number[] = [];
  const listenerCount = () => listeners.length;

  const win: PageWindow = {
    addEventListener(_type, listener) {
      listeners.push(listener);
    },
    removeEventListener(_type, listener) {
      const at = listeners.indexOf(listener);
      if (at >= 0) listeners.splice(at, 1);
    },
  };
  if (bridge !== "none") {
    win.__ubjsElectron = {
      protocol: bridge as typeof PROTOCOL,
      connect(id: string) {
        listenersAtAsk.push(listenerCount());
        ids.push(id);
      },
    };
  }

  return {
    win,
    ids,
    listenersAtAsk,
    dispatch(
      data: unknown,
      opts: { source?: unknown; ports?: unknown[] } = {},
    ) {
      const ev = {
        data,
        source: opts.source ?? win,
        ports: opts.ports ?? [],
      } as unknown as MessageEvent;
      for (const listener of [...listeners]) listener(ev);
    },
    listenerCount,
  };
}
