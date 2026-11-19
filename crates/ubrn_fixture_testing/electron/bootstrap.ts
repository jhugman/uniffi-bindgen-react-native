/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The page: connects through @ubjs/electron, then runs the fixture's script.
import { connect } from "@ubjs/electron/page";
import { uniffiInitAsync } from "@/generated/index.port";
import {
  exitOf,
  fixture,
  pageIndex,
  setConnection,
  settled,
  utilityPid,
} from "@/asserts";

const errors: string[] = [];
addEventListener("error", (ev) => errors.push(String(ev.error ?? ev.message)));
addEventListener("unhandledrejection", (ev) =>
  errors.push(`unhandled rejection: ${String(ev.reason)}`),
);

(async () => {
  try {
    // A reload's old utility must end itself once its port closes. Waiting for
    // that before connecting means a retire for this connect cannot stand in.
    const page = await pageIndex();
    if (page > 0) await exitOf(await utilityPid(page - 1));
    const start = performance.now();
    const conn = await connect({ timeoutMs: 15_000 });
    await uniffiInitAsync(conn.port);
    const startupMs = Math.round(performance.now() - start);
    setConnection(conn);
    // After init: the script calls into the library as it loads.
    await import("@fixture/script");
    const result = await settled();
    fixture.done({
      ...result,
      startupMs,
      failures: [...result.failures, ...errors],
    });
  } catch (e) {
    const failure = e instanceof Error ? (e.stack ?? e.message) : String(e);
    fixture.done({ ran: 0, failures: [...errors, `bootstrap: ${failure}`] });
  }
})();
