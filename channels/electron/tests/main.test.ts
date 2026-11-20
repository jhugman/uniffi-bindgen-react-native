/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { EventEmitter } from "node:events";
import { test } from "node:test";
import type { WebContents, WebFrameMain } from "electron";
import {
  createMain,
  sharedMain,
  type MainApi,
  type ServeOptions,
  type UtilityEvent,
} from "../src/main/core.js";
import { sendToFrame } from "../src/main/frame.js";
import { inspectArgv, unpackedPath } from "../src/main/paths.js";
import { CONNECT, INIT, PORT, type PortReply } from "../src/protocol.js";
import { FakeFrame, FakeWebContents, fakeMain } from "./fakes/main.js";
import { ID } from "./fakes/helpers.js";

const asWc = (wc: FakeWebContents) => wc as unknown as WebContents;
const asFrame = (frame: FakeFrame) => frame as unknown as WebFrameMain;

const PATHS = { entry: "/app/utility.cjs", library: "/app/lib.dylib" };

function setup(options: Partial<ServeOptions> = {}, fx = fakeMain()) {
  const main = createMain(fx.deps);
  const wc = new FakeWebContents(1);
  const events: UtilityEvent[] = [];
  const handle = main.serve(asWc(wc), {
    ...PATHS,
    info: () => ({ dataDir: "/data" }),
    onUtility: (e) => void events.push(e),
    ...options,
  });
  const ask = (
    frame: FakeFrame | null = wc.mainFrame,
    payload: unknown = { id: ID },
  ) => fx.request(wc, frame, payload);
  return { fx, main, wc, frame: wc.mainFrame, events, handle, ask };
}

/** The one reply a frame was posted, which must be an error. */
function errorReply(frame: FakeFrame) {
  assert.strictEqual(frame.posted.length, 1, "one reply");
  const [{ channel, message, ports }] = frame.posted;
  assert.strictEqual(channel, PORT);
  assert.deepStrictEqual(ports, []);
  const reply = message as PortReply;
  assert.strictEqual(reply.id, ID);
  assert.ok("error" in reply, "an error reply");
  assert.strictEqual(typeof reply.error.message, "string");
  return reply.error;
}

test("an unserved webContents is told not-served at once", () => {
  const fx = fakeMain();
  createMain(fx.deps);
  const wc = new FakeWebContents(7);
  fx.request(wc, wc.mainFrame, { id: ID });

  const error = errorReply(wc.mainFrame);
  assert.deepStrictEqual(wc.mainFrame.posted, [
    {
      channel: PORT,
      message: {
        id: ID,
        error: { reason: "not-served", message: error.message },
      },
      ports: [],
    },
  ]);
  assert.match(error.message, /serve\(\)/);
  assert.deepStrictEqual(fx.forks, []);
});

test("a request with no sender frame, or a malformed id, is dropped", () => {
  const s = setup();
  s.ask(null);
  for (const payload of [
    undefined,
    null,
    ID,
    {},
    { id: "x" },
    { id: ID.toUpperCase() },
  ]) {
    s.fx.request(s.wc, s.frame, payload);
  }
  assert.deepStrictEqual(s.fx.forks, []);
  assert.deepStrictEqual(s.frame.posted, []);
  assert.deepStrictEqual(s.frame.sent, []);

  // Dropped before the registry is asked, so not even not-served.
  const unserved = new FakeWebContents(2);
  s.fx.request(unserved, unserved.mainFrame, { id: "x" });
  s.fx.request(unserved, null, { id: ID });
  assert.deepStrictEqual(unserved.mainFrame.posted, []);
});

test("by default only the main frame may ask; allowFrame decides otherwise", () => {
  const sub = new FakeFrame("file:///app/sub.html");
  // A declined frame hears not-served at once rather than waiting out its timeout.
  const declined = (frame: FakeFrame) => {
    const error = errorReply(frame);
    assert.strictEqual(error.reason, "not-served");
    assert.match(error.message, /allowFrame/);
  };

  const s = setup();
  s.ask(sub);
  assert.deepStrictEqual(s.fx.forks, []);
  declined(sub);
  assert.deepStrictEqual(s.frame.posted, []);
  s.ask();
  assert.strictEqual(s.fx.forks.length, 1);

  const t = setup({ allowFrame: (f) => f.url === "file:///app/sub.html" });
  t.ask();
  assert.deepStrictEqual(t.fx.forks, []);
  declined(t.frame);
  t.ask(sub);
  assert.strictEqual(t.fx.forks.length, 1);

  // An app's filter that throws declines, and main carries on.
  const u = setup({
    allowFrame: () => {
      throw new Error("bad filter");
    },
  });
  assert.doesNotThrow(() => u.ask());
  assert.deepStrictEqual(u.fx.forks, []);
  declined(u.frame);
  assert.strictEqual(u.fx.warnings.length, 1);
  assert.match(u.fx.warnings[0], /bad filter/);
});

test("inspect adds a debugger flag, and a packaged app ignores it with a warning", () => {
  const warnings: string[] = [];
  const warn = (m: string) => void warnings.push(m);
  assert.deepStrictEqual(inspectArgv(undefined, false, warn), []);
  assert.deepStrictEqual(inspectArgv(false, false, warn), []);
  assert.deepStrictEqual(inspectArgv(true, false, warn), ["--inspect=0"]);
  assert.deepStrictEqual(warnings, []);

  const s = setup({ inspect: true });
  s.ask();
  assert.deepStrictEqual(s.fx.forks[0].options.execArgv, ["--inspect=0"]);

  const fx = fakeMain();
  fx.setPackaged(true);
  const t = setup({ inspect: true }, fx);
  t.ask();
  assert.deepStrictEqual(t.fx.forks[0].options.execArgv, []);
  assert.strictEqual(fx.warnings.length, 1);
  assert.match(fx.warnings[0], /inspect/);
});

test("ready sends the page its port and info", () => {
  const s = setup();
  s.ask();
  const [{ child }] = s.fx.forks;
  const [{ port2 }] = s.fx.channels;

  child.spawn(4242);
  assert.strictEqual(s.frame.posted.length, 0);
  child.ready();

  assert.strictEqual(s.frame.posted.length, 1);
  const [post] = s.frame.posted;
  assert.strictEqual(post.channel, PORT);
  assert.deepStrictEqual(post.message, { id: ID, info: { dataDir: "/data" } });
  assert.strictEqual(post.ports.length, 1);
  assert.strictEqual(post.ports[0], port2);

  assert.deepStrictEqual(s.events, [
    { kind: "spawn", id: ID, pid: 4242 },
    { kind: "ready", id: ID, pid: 4242 },
  ]);

  // Only the first ready counts.
  child.ready();
  assert.strictEqual(s.frame.posted.length, 1);
  assert.strictEqual(s.events.length, 2);

  // An app's onUtility that throws is warned, and the page still gets its port.
  const t = setup({
    onUtility: () => {
      throw new Error("bad listener");
    },
  });
  t.ask();
  const tChild = t.fx.forks[0].child;
  assert.doesNotThrow(() => {
    tChild.spawn(1);
    tChild.ready();
  });
  assert.strictEqual(t.frame.posted.length, 1);
  assert.strictEqual(t.fx.warnings.length, 2);
  for (const w of t.fx.warnings) assert.match(w, /bad listener/);
});

test("info that throws, or that structured clone refuses, fails the start and forks nothing", () => {
  const s = setup({
    info: () => {
      throw new Error("no dir");
    },
  });
  assert.doesNotThrow(() => s.ask());
  const error = errorReply(s.frame);
  assert.strictEqual(error.reason, "start-failed");
  assert.match(error.message, /no dir/);
  assert.deepStrictEqual(s.fx.forks, []);

  for (const info of [
    () => ({ f() {} }),
    () => ({ dir: new URL("file:///data") }),
  ]) {
    const t = setup({ info });
    assert.doesNotThrow(() => t.ask());
    assert.strictEqual(errorReply(t.frame).reason, "start-failed");
    assert.deepStrictEqual(t.fx.forks, []);
  }
});

test("a fork that throws fails the start", () => {
  const fx = fakeMain();
  fx.failFork(new Error("spawn EACCES"));
  const s = setup({}, fx);
  assert.doesNotThrow(() => s.ask());
  const error = errorReply(s.frame);
  assert.strictEqual(error.reason, "start-failed");
  assert.match(error.message, /spawn EACCES/);
  // No ports were made, so none are left open.
  assert.deepStrictEqual(fx.channels, []);
});

test("an init that throws fails the start and retires the child, even before it spawns", () => {
  const fx = fakeMain();
  fx.failInit(new TypeError("An object could not be cloned."));
  const s = setup({}, fx);
  assert.doesNotThrow(() => s.ask());
  const error = errorReply(s.frame);
  assert.strictEqual(error.reason, "start-failed");
  assert.match(error.message, /could not be cloned/);

  const [{ child }] = fx.forks;
  const [{ port1, port2 }] = fx.channels;
  assert.ok(port1.closed && port2.closed, "both ports closed");
  // Not yet spawned, so that kill did nothing; it is killed again on spawn.
  assert.strictEqual(child.killed, 1);
  child.spawn(7);
  assert.strictEqual(child.killed, 2);

  // Like any failed start: a new request waits for its exit, which is reported.
  s.ask(s.frame, { id: "f".repeat(32) });
  assert.strictEqual(fx.forks.length, 1, "no fork alongside it");
  child.exit(1);
  assert.strictEqual(fx.forks.length, 2);
  assert.deepStrictEqual(
    s.events.map((e) => [e.kind, "expected" in e ? e.expected : undefined]),
    [
      ["spawn", undefined],
      ["exit", false],
    ],
  );
});

test("a utility's fatal error is warned, not thrown", () => {
  const s = setup();
  s.ask();
  const [{ child }] = s.fx.forks;
  assert.doesNotThrow(() =>
    child.emit("error", "FatalError", "v8::ToLocalChecked", "{}"),
  );
  assert.strictEqual(s.fx.warnings.length, 1);
  assert.match(s.fx.warnings[0], /FatalError at v8::ToLocalChecked/);
});

test("serve checks it has a live webContents, and its paths: absolute, existing, and unpacked from app.asar", () => {
  const fx = fakeMain();
  const main = createMain(fx.deps);
  const serve = (id: number, paths: Partial<ServeOptions>) =>
    main.serve(asWc(new FakeWebContents(id)), { ...PATHS, ...paths });

  // A BrowserWindow has an id and events, but no mainFrame.
  const win = Object.assign(new EventEmitter(), {
    id: 1,
    webContents: new FakeWebContents(1),
  });
  for (const notWc of [win, undefined, null, 1]) {
    assert.throws(
      () => main.serve(notWc as unknown as WebContents, PATHS),
      (e: unknown) =>
        e instanceof TypeError && /win\.webContents/.test(e.message),
    );
  }
  fx.request(win.webContents, win.webContents.mainFrame, { id: ID });
  assert.strictEqual(
    errorReply(win.webContents.mainFrame).reason,
    "not-served",
  );

  assert.throws(() => serve(1, { entry: "utility.cjs" }), TypeError);
  assert.throws(() => serve(2, { library: "lib.dylib" }), TypeError);
  assert.throws(
    () => serve(3, { library: "/app/missing.dylib" }),
    (e: unknown) =>
      e instanceof Error &&
      !(e instanceof TypeError) &&
      e.message.includes("/app/missing.dylib"),
  );
  // A destroyed webContents would never fire destroyed again to unserve it.
  const gone = new FakeWebContents(6);
  gone.destroyed = true;
  assert.throws(() => main.serve(asWc(gone), PATHS), /already destroyed/);

  // A serve that threw serves nothing.
  const refused = new FakeWebContents(3);
  fx.request(refused, refused.mainFrame, { id: ID });
  assert.strictEqual(errorReply(refused.mainFrame).reason, "not-served");

  // The library is rewritten and checked; the entry may stay in the archive.
  fx.files.add("/A/Resources/app.asar.unpacked/node_modules/x/lib.dylib");
  const wc = new FakeWebContents(4);
  main.serve(asWc(wc), {
    entry: "/A/Resources/app.asar/main/utility.cjs",
    library: "/A/Resources/app.asar/node_modules/x/lib.dylib",
  });
  fx.request(wc, wc.mainFrame, { id: ID });
  const [{ entry, child }] = fx.forks;
  assert.strictEqual(entry, "/A/Resources/app.asar/main/utility.cjs");
  assert.strictEqual(
    (child.posted[0].message as { library: string }).library,
    "/A/Resources/app.asar.unpacked/node_modules/x/lib.dylib",
  );
  // In an archive with no unpacked copy, the error names the unpacked path.
  assert.throws(
    () => serve(5, { library: "/A/Resources/app.asar/y.dylib" }),
    /app\.asar\.unpacked\/y\.dylib/,
  );

  assert.strictEqual(
    unpackedPath("/A/Resources/app.asar/node_modules/x/lib.dylib"),
    "/A/Resources/app.asar.unpacked/node_modules/x/lib.dylib",
  );
  assert.strictEqual(
    unpackedPath("C:\\A\\app.asar\\x.dll"),
    "C:\\A\\app.asar.unpacked\\x.dll",
  );
  for (const path of [
    "/A/app.asar.unpacked/x.dylib",
    "/A/my-app.asar/x.dylib",
    "/A/lib.dylib",
  ]) {
    assert.strictEqual(unpackedPath(path), path);
  }
});

test("a webContents is served once, and close() or destroyed stops serving it", () => {
  const s = setup();
  assert.throws(() => s.main.serve(asWc(s.wc), PATHS), /already/);

  s.handle.close();
  s.handle.close();
  assert.strictEqual(s.wc.listenerCount("destroyed"), 0);
  s.ask();
  assert.strictEqual(errorReply(s.frame).reason, "not-served");
  assert.deepStrictEqual(s.fx.forks, []);

  // Served again, a stale handle's close() leaves the new serve alone.
  s.frame.posted = [];
  s.main.serve(asWc(s.wc), PATHS);
  s.handle.close();
  s.ask();
  assert.deepStrictEqual(s.frame.posted, []);
  assert.strictEqual(s.fx.forks.length, 1);

  const wc2 = new FakeWebContents(2);
  s.main.serve(asWc(wc2), PATHS);
  wc2.emit("destroyed");
  s.fx.request(wc2, wc2.mainFrame, { id: ID });
  assert.strictEqual(errorReply(wc2.mainFrame).reason, "not-served");
  assert.strictEqual(s.fx.forks.length, 1);
});

test("a reply that throws is caught and warned", () => {
  const fx = fakeMain();
  createMain(fx.deps);
  const wc = new FakeWebContents(3);
  wc.mainFrame.failSends = true;
  assert.doesNotThrow(() => fx.request(wc, wc.mainFrame, { id: ID }));
  assert.deepStrictEqual(wc.mainFrame.posted, []);
  assert.strictEqual(fx.warnings.length, 1);

  // The start-failed reply and the port go the same way.
  const s = setup({
    info: () => {
      throw new Error("no dir");
    },
  });
  s.frame.failSends = true;
  assert.doesNotThrow(() => s.ask());
  assert.strictEqual(s.fx.warnings.length, 1);
  // A refused port is warned once, and retires the connection.
  const t = setup();
  t.ask();
  t.frame.failSends = true;
  const { child } = t.fx.forks[0];
  child.spawn(1);
  assert.doesNotThrow(() => child.ready());
  assert.deepStrictEqual(t.frame.posted, []);
  assert.strictEqual(t.fx.warnings.length, 1);
  assert.strictEqual(child.killed, 1);

  const owner = new FakeWebContents(4);
  /** Whether sendToFrame calls `send` for `frame`; it must not warn. */
  const reaches = (frame: FakeFrame) => {
    const warnings: string[] = [];
    let called = false;
    const sent = sendToFrame(
      asWc(owner),
      asFrame(frame),
      (m) => void warnings.push(m),
      () => {
        called = true;
      },
    );
    assert.strictEqual(sent, called);
    assert.deepStrictEqual(warnings, []);
    return sent;
  };
  // A destroyed frame, or a detached one that a navigation replaced, is skipped.
  for (const flag of ["destroyed", "detached"] as const) {
    const frame = new FakeFrame();
    frame[flag] = true;
    assert.strictEqual(reaches(frame), false, flag);
  }
  // Electron leaves detached set on the main frame a reload re-hosts after a
  // renderer crash, and a send to it lands.
  owner.mainFrame.detached = true;
  assert.strictEqual(reaches(owner.mainFrame), true, "re-hosted main frame");
  owner.destroyed = true;
  assert.strictEqual(reaches(owner.mainFrame), false, "destroyed webContents");
  owner.destroyed = false;
  owner.mainFrame.destroyed = true;
  assert.strictEqual(reaches(owner.mainFrame), false, "destroyed main frame");

  const live = new FakeFrame();
  assert.strictEqual(
    sendToFrame(asWc(owner), asFrame(live), assert.fail, (f) => f.send("x", 1)),
    true,
  );
  assert.deepStrictEqual(live.sent, [{ channel: "x", args: [1] }]);
});
