/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// "@/asserts" for a fixture script in an Electron page. A page has no process
// exit to say a script is done, so this counts the tests it starts. The
// lifecycle script also drives main through the helpers at the end.
import * as base from "../../../typescript/testing/asserts";
import type { Connection } from "@ubjs/electron/page";

export * from "../../../typescript/testing/asserts";

/** What a page reports to main once its tests have settled. */
export interface PageResult {
  ran: number;
  failures: string[];
  startupMs?: number;
  next?: "reload" | "close";
}

/** The test-only bridge the harness's preload exposes. */
interface Fixture {
  sandboxed: boolean;
  contextIsolated: boolean;
  page(): Promise<number>;
  utilityPid(page: number): Promise<number>;
  exitOf(pid: number): Promise<{ code: number }>;
  killUtility(): Promise<number>;
  done(result: PageResult): void;
}

export const fixture = (globalThis as unknown as { __fixture: Fixture })
  .__fixture;

const pending = new Set<Promise<unknown>>();
const failures: string[] = [];
let ran = 0;
let next: PageResult["next"];
let conn: Connection | undefined;

const describe = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function test<T>(name: string, block: (t: base.Asserts) => T): T {
  ran += 1;
  try {
    return base.test(name, block);
  } catch (e) {
    failures.push(`${name}: ${describe(e)}`);
    throw e;
  }
}

export function asyncTest<T>(
  name: string,
  block: (t: base.AsyncAsserts) => Promise<T>,
  timeout?: number,
): Promise<T> {
  ran += 1;
  const p = base.asyncTest(name, block, timeout);
  pending.add(p);
  p.then(
    () => pending.delete(p),
    (e) => {
      pending.delete(p);
      failures.push(`${name}: ${describe(e)}`);
    },
  );
  return p;
}

/** Resolves once no test is running and none starts in a quiet period. */
export async function settled(): Promise<PageResult> {
  for (;;) {
    await Promise.allSettled([...pending]);
    await new Promise((resolve) => setTimeout(resolve, 100));
    if (pending.size === 0) return { ran, failures, next };
  }
}

export function setConnection(connection: Connection): void {
  conn = connection;
}

/** The page's connection, which the bootstrap made before the script ran. */
export function connection(): Connection {
  if (!conn) throw new Error("no connection");
  return conn;
}

/** Which load of the window this is: 0, then 1 after a reload, and so on. */
export const pageIndex = () => fixture.page();
export const utilityPid = (page: number) => fixture.utilityPid(page);
/** The exit of a utility, once it has exited. */
export const exitOf = (pid: number) => fixture.exitOf(pid);
/** SIGKILLs this page's utility from outside @ubjs/electron, as a crash would. */
export const killUtility = () => fixture.killUtility();

/** What main does once this page's tests settle: reload it, or close the
 * window and check that its utility ends. */
export function afterThisPage(action: "reload" | "close"): void {
  next = action;
}
