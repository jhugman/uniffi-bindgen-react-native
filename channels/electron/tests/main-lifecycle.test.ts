/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { test, type TestContext } from "node:test";
import type { WebContents } from "electron";
import {
  createMain,
  type MainApi,
  type ServeOptions,
  type UtilityEvent,
} from "../src/main/core.js";
import {
  DISCONNECTED,
  PORT,
  type InitMessage,
  type PortReply,
} from "../src/protocol.js";
import {
  FakeFrame,
  FakeWebContents,
  fakeMain,
  type FakeUtility,
} from "./fakes/main.js";

const ID1 = "11111111111111111111111111111111";
const ID2 = "22222222222222222222222222222222";
const ID3 = "33333333333333333333333333333333";

const PATHS = { entry: "/app/utility.cjs", library: "/app/lib.dylib" };

type Fx = ReturnType<typeof fakeMain>;

/** Serves a new webContents `wcId` on `main`. */
function serveOn(
  fx: Fx,
  main: MainApi,
  wcId: number,
  options: Partial<ServeOptions> = {},
) {
  const wc = new FakeWebContents(wcId);
  const events: UtilityEvent[] = [];
  const handle = main.serve(wc as unknown as WebContents, {
    ...PATHS,
    info: () => ({ dataDir: "/data" }),
    onUtility: (e) => void events.push(e),
    ...options,
  });
  const ask = (id: string, frame: FakeFrame = wc.mainFrame) =>
    fx.request(wc, frame, { id });
  /** Asks for `id`, which must fork at once, and plays its utility to ready. */
  const live = (id: string, pid: number, frame?: FakeFrame) => {
    const before = fx.forks.length;
    ask(id, frame);
    assert.strictEqual(fx.forks.length, before + 1, `${id} forked at once`);
    const { child } = fx.forks[before];
    child.spawn(pid);
    child.ready();
    return child;
  };
  const exits = () => events.filter((e) => e.kind === "exit");
  return { wc, frame: wc.mainFrame, events, handle, ask, live, exits };
}

function setup(options: Partial<ServeOptions> = {}, fx = fakeMain()) {
  const main = createMain(fx.deps);
  return { fx, main, ...serveOn(fx, main, 1, options) };
}

/** Asks for `id` in a fresh setup and spawns its utility, which stays starting. */
function starting(options: Partial<ServeOptions> = {}, fx = fakeMain()) {
  const s = setup(options, fx);
  s.ask(ID1);
  const { child } = s.fx.forks[0];
  child.spawn(101);
  return { ...s, child };
}

function initId(child: FakeUtility): string {
  return (child.posted[0].message as InitMessage).id;
}

/** Every error reply a frame was posted, as { id, reason, message }. */
function errorReplies(frame: FakeFrame) {
  return frame.posted.flatMap(({ channel, message, ports }) => {
    assert.strictEqual(channel, PORT);
    const reply = message as PortReply;
    if (!("error" in reply)) return [];
    assert.deepStrictEqual(ports, []);
    return [{ id: reply.id, ...reply.error }];
  });
}

function disconnected(id: string, code: number) {
  return {
    channel: DISCONNECTED,
    args: [{ id, code, reason: "utility-exited" }],
  };
}

function exitEvent(
  id: string,
  pid: number | undefined,
  code: number,
  expected: boolean,
): UtilityEvent {
  return { kind: "exit", id, pid, code, expected };
}

const mockTimers = (t: TestContext) =>
  t.mock.timers.enable({ apis: ["setTimeout"] });

test("a new request retires the current connection before forking", () => {
  const s = setup();
  const child1 = s.live(ID1, 101);
  s.ask(ID2);
  assert.strictEqual(child1.killed, 1);
  assert.strictEqual(s.fx.forks.length, 1, "no fork until the old one exits");

  child1.exit(15);
  assert.strictEqual(s.fx.forks.length, 2);
  const child2 = s.fx.forks[1].child;
  assert.strictEqual(initId(child2), ID2);
  assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, 15, true)]);
  // After a real reload, the new document's preload drops it: ID1 is not its id.
  assert.deepStrictEqual(s.frame.sent, [disconnected(ID1, 15)]);

  child2.spawn(102);
  assert.deepStrictEqual(
    s.events.map((e) => `${e.kind} ${e.id}`),
    [`spawn ${ID1}`, `ready ${ID1}`, `exit ${ID1}`, `spawn ${ID2}`],
  );
});

test("a utility retired before it spawned is killed when it spawns", () => {
  const s = setup();
  s.ask(ID1);
  const early = s.fx.forks[0].child;
  s.ask(ID2);
  assert.strictEqual(early.killed, 1, "a kill before spawn does nothing");
  early.spawn(301);
  assert.strictEqual(early.killed, 2, "killed again on spawn");
  assert.strictEqual(s.fx.forks.length, 1, "still waiting for its exit");
  early.exit(15);
  assert.strictEqual(s.fx.forks.length, 2);
});

test("a utility retired twice is killed once and its page told once", () => {
  const s = starting();
  s.ask(ID2);
  s.handle.close();
  assert.strictEqual(s.child.killed, 1);
  // close() also answers the waiting ID2; ID1 hears only that it was replaced.
  assert.deepStrictEqual(
    errorReplies(s.frame)
      .filter((r) => r.id === ID1)
      .map((r) => r.reason),
    ["start-failed"],
  );
});

test("three requests in a row fork once more, for the newest id", () => {
  const s = setup();
  const child1 = s.live(ID1, 101);
  s.ask(ID2);
  s.ask(ID3);
  // ID2 is told at once, not when the old utility exits.
  assert.strictEqual(child1.killed, 1);
  const [replaced] = errorReplies(s.frame);
  assert.strictEqual(replaced.id, ID2);
  assert.strictEqual(replaced.reason, "start-failed");
  assert.match(replaced.message, /replaced/);

  child1.exit(15);
  assert.strictEqual(s.fx.forks.length, 2);
  assert.strictEqual(initId(s.fx.forks[1].child), ID3);
  assert.deepStrictEqual(errorReplies(s.frame), [replaced]);
  assert.ok(!s.events.some((e) => e.id === ID2), "ID2 never forked");
});

test("an error message while starting fails the start with the utility's message, warns its stack, and its exit is not expected", () => {
  const s = starting();
  s.child.fail("panic in register");
  assert.deepStrictEqual(errorReplies(s.frame), [
    { id: ID1, reason: "start-failed", message: "panic in register" },
  ]);
  assert.deepStrictEqual(s.fx.warnings, [
    "the utility failed to start: Error: panic in register\n    at register",
  ]);
  assert.strictEqual(s.child.killed, 1);

  s.child.exit(1);
  assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, 1, false)]);
  assert.strictEqual(s.frame.posted.length, 1);
  assert.deepStrictEqual(s.frame.sent, []);
  // A ready after the error is too late, and a second error is not warned.
  s.child.ready();
  s.child.fail("again");
  assert.strictEqual(s.frame.posted.length, 1);
  assert.strictEqual(s.fx.warnings.length, 1);
});

test("an exit while starting fails the start with its code, and is not expected, even with code 0", () => {
  for (const code of [1, 0]) {
    const s = starting();
    s.child.exit(code);
    const replies = errorReplies(s.frame);
    assert.strictEqual(replies.length, 1);
    assert.strictEqual(replies[0].id, ID1);
    assert.strictEqual(replies[0].reason, "start-failed");
    assert.match(replies[0].message, new RegExp(`code ${code}\\b`));
    assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, code, false)]);
    assert.deepStrictEqual(s.frame.sent, []);
    // It is gone, so the next request forks at once.
    s.ask(ID2);
    assert.strictEqual(s.fx.forks.length, 2);
  }
});

test("ready for a destroyed or detached frame retires instead of replying", () => {
  for (const flag of ["destroyed", "detached"] as const) {
    const s = starting();
    s.frame[flag] = true;
    // A navigation detaches the frame it replaces.
    if (flag === "detached") s.wc.mainFrame = new FakeFrame();
    s.child.ready();
    assert.deepStrictEqual(s.frame.posted, [], flag);
    assert.strictEqual(s.child.killed, 1, flag);
    assert.ok(s.fx.channels[0].port2.closed, `${flag}: main's end closed`);
    assert.ok(!s.events.some((e) => e.kind === "ready"), flag);

    s.child.exit(15);
    assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, 15, true)], flag);
    assert.deepStrictEqual(s.frame.sent, [], flag);
    assert.deepStrictEqual(s.fx.warnings, [], flag);
  }
});

test("a main frame that Electron leaves detached after a crash still hears from main", () => {
  // Electron sets detached at the crash and keeps it once a reload re-hosts
  // the frame, though the frame is still wc.mainFrame and a send to it lands.
  const s = setup();
  const crashed = s.live(ID1, 101);
  s.wc.emit("render-process-gone");
  s.frame.detached = true;
  crashed.exit(0);
  const child = s.live(ID2, 102);
  const ports = s.frame.posted.filter(
    (p) => !("error" in (p.message as PortReply)),
  );
  assert.strictEqual(ports.length, 2, "ID1's port, then ID2's");
  assert.deepStrictEqual(ports[1].message, {
    id: ID2,
    info: { dataDir: "/data" },
  });
  assert.deepStrictEqual(ports[1].ports, [s.fx.channels[1].port2]);
  assert.ok(s.events.some((e) => e.kind === "ready" && e.id === ID2));
  child.exit(9);
  assert.deepStrictEqual(s.frame.sent, [disconnected(ID2, 9)]);

  // A start failure reaches it too.
  const u = starting();
  u.frame.detached = true;
  u.child.fail("panic in register");
  assert.deepStrictEqual(errorReplies(u.frame), [
    { id: ID1, reason: "start-failed", message: "panic in register" },
  ]);
  assert.strictEqual(u.fx.warnings.length, 1, "only the stack");
});

test("a destroyed frame is sent nothing, and a send that throws is caught, on every path", (t) => {
  mockTimers(t);
  const startFailures: [string, (child: FakeUtility) => void][] = [
    ["error", (child) => child.fail("panic in register")],
    ["exit", (child) => child.exit(1)],
  ];
  const flags = ["destroyed", "failSends"] as const;

  for (const flag of flags) {
    const warnings = flag === "failSends" ? 1 : 0;
    for (const [name, fail] of startFailures) {
      const what = `${flag}, ${name}`;
      // An error message also warns its stack.
      const warned = warnings + (name === "error" ? 1 : 0);
      const s = starting();
      s.frame[flag] = true;
      assert.doesNotThrow(() => fail(s.child), what);
      // Retired: killed unless it exited, and its exit is a failed start.
      if (s.child.pid !== undefined) {
        assert.strictEqual(s.child.killed, 1, what);
        s.child.exit(1);
      }
      assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, 1, false)], what);
      assert.deepStrictEqual(s.frame.posted, [], what);
      assert.deepStrictEqual(s.frame.sent, [], what);
      assert.strictEqual(s.fx.warnings.length, warned, what);
    }

    // A live exit. Electron's send() logs a failure itself.
    const l = setup();
    const child = l.live(ID1, 101);
    l.frame[flag] = true;
    assert.doesNotThrow(() => child.exit(9), flag);
    assert.deepStrictEqual(l.frame.sent, [], flag);
    assert.strictEqual(l.fx.warnings.length, 0, flag);
    assert.strictEqual(l.frame.consoleErrors.length, warnings, flag);

    // Replaced and closed while starting.
    const r = starting();
    r.frame[flag] = true;
    assert.doesNotThrow(() => r.ask(ID2), flag);
    assert.strictEqual(r.child.killed, 1, flag);
    assert.deepStrictEqual(r.frame.posted, [], flag);
    assert.strictEqual(r.fx.warnings.length, warnings, flag);

    const c = starting();
    c.frame[flag] = true;
    assert.doesNotThrow(() => c.handle.close(), flag);
    assert.strictEqual(c.child.killed, 1, flag);
    assert.deepStrictEqual(c.frame.posted, [], flag);
    assert.strictEqual(c.fx.warnings.length, warnings, flag);
  }

  // A PORT that throws is warned, and retires the connection as a gone frame does.
  const p = starting();
  p.frame.failSends = true;
  assert.doesNotThrow(() => p.child.ready());
  assert.strictEqual(p.fx.warnings.length, 1);
  assert.strictEqual(p.child.killed, 1);
  assert.ok(p.fx.channels[0].port2.closed, "main's end closed");
  p.child.exit(15);
  assert.deepStrictEqual(p.exits(), [exitEvent(ID1, 101, 15, true)]);
});

test("render-process-gone, destroyed and close() each kill the utility, and close() still tells a live page", () => {
  const ends: [string, (s: ReturnType<typeof setup>) => void][] = [
    ["render-process-gone", (s) => s.wc.emit("render-process-gone")],
    ["destroyed", (s) => s.wc.emit("destroyed")],
    ["close()", (s) => s.handle.close()],
  ];
  for (const [name, end] of ends) {
    const s = setup();
    const child = s.live(ID1, 101);
    end(s);
    assert.strictEqual(child.killed, 1, name);
  }

  // A navigation alone retires nothing: the next request does.
  const n = setup();
  const navigated = n.live(ID1, 101);
  n.wc.emit("did-start-navigation");
  n.wc.emit("did-navigate");
  assert.strictEqual(navigated.killed, 0);

  // A crash drops the request waiting on the old utility: its document is gone.
  const d = setup();
  const before = d.live(ID1, 101);
  d.ask(ID2);
  d.wc.emit("render-process-gone");
  before.exit(15);
  assert.strictEqual(d.fx.forks.length, 1);
  assert.deepStrictEqual(errorReplies(d.frame), []);

  // A crashed renderer's window is still served once it reloads.
  const g = setup();
  const crashed = g.live(ID1, 101);
  g.wc.emit("render-process-gone");
  crashed.exit(15);
  g.ask(ID2);
  assert.strictEqual(g.fx.forks.length, 2);

  const s = setup();
  const child = s.live(ID1, 101);
  s.handle.close();
  assert.strictEqual(s.wc.listenerCount("render-process-gone"), 0);
  child.exit(15);
  assert.deepStrictEqual(s.frame.sent, [disconnected(ID1, 15)]);
  assert.deepStrictEqual(s.exits(), [exitEvent(ID1, 101, 15, true)]);

  const u = starting();
  u.handle.close();
  const replies = errorReplies(u.frame);
  assert.strictEqual(replies.length, 1);
  assert.strictEqual(replies[0].id, ID1);
  assert.strictEqual(replies[0].reason, "start-failed");
  assert.match(replies[0].message, /serve\(\) was closed/);
  assert.strictEqual(u.child.killed, 1);
  u.child.exit(15);
  assert.deepStrictEqual(u.exits(), [exitEvent(ID1, 101, 15, true)]);
  assert.strictEqual(u.frame.posted.length, 1);

  // A request still waiting for the old utility is told too, and never forks.
  const w = setup();
  const old = w.live(ID1, 101);
  w.ask(ID2);
  w.handle.close();
  const [closed] = errorReplies(w.frame);
  assert.strictEqual(closed.id, ID2);
  assert.match(closed.message, /serve\(\) was closed/);
  old.exit(15);
  assert.strictEqual(w.fx.forks.length, 1);
});

test("two served windows: a utility's death tells only its own frame", () => {
  const fx = fakeMain();
  const main = createMain(fx.deps);
  const a = serveOn(fx, main, 1);
  const b = serveOn(fx, main, 2);
  const child1 = a.live(ID1, 101);
  const child2 = b.live(ID2, 102);

  b.wc.emit("destroyed");
  assert.strictEqual(child2.killed, 1);
  assert.strictEqual(child1.killed, 0);

  child1.exit(9);
  assert.deepStrictEqual(a.frame.sent, [disconnected(ID1, 9)]);
  assert.deepStrictEqual(b.frame.sent, []);
  assert.deepStrictEqual(a.exits(), [exitEvent(ID1, 101, 9, false)]);
  assert.deepStrictEqual(b.exits(), []);

  child2.exit(15);
  assert.deepStrictEqual(b.frame.sent, []);
  assert.deepStrictEqual(b.exits(), [exitEvent(ID2, 102, 15, true)]);
  assert.strictEqual(a.frame.sent.length, 1);
});

test("a late ready from a retired connection is ignored", () => {
  const s = starting();
  s.ask(ID2);
  assert.strictEqual(s.child.killed, 1);
  s.child.ready();

  const replies = errorReplies(s.frame);
  assert.strictEqual(s.frame.posted.length, 1, "no PORT");
  assert.strictEqual(replies[0].id, ID1);
  assert.match(replies[0].message, /replaced/);
  assert.ok(!s.events.some((e) => e.kind === "ready"));
  assert.strictEqual(s.child.killed, 1);
});

test("serving a webContents again after close() still waits for its old utility", () => {
  const s = setup();
  const old = s.live(ID1, 101);
  s.handle.close();
  s.main.serve(s.wc as unknown as WebContents, PATHS);
  s.ask(ID2);
  assert.strictEqual(s.fx.forks.length, 1);
  old.exit(15);
  assert.strictEqual(s.fx.forks.length, 2);
  assert.strictEqual(initId(s.fx.forks[1].child), ID2);
});
