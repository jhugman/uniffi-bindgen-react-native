/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// One fixture's main process: serves its page through @ubjs/electron,
// reloads or closes the window when the page asks, and prints the result.
const { app, BrowserWindow, ipcMain, session } = require("electron");
const path = require("node:path");

const OUT = process.env.FIXTURE_OUT;
const WATCHDOG_MS = 120_000;
// How long a utility may take to exit once its page has gone.
const EXIT_WITHIN_MS = 5000;

const result = { ran: 0, failures: [], startupMs: [], pids: [] };
const events = [];
const waiters = [];
// The utility each load of the window connected to.
const pidOfPage = [];
let page = 0;
let win;
let finished = false;

function finish(failure) {
  if (finished) return;
  finished = true;
  if (failure) result.failures.push(failure);
  const passed = result.failures.length === 0 && result.ran > 0;
  // Pipes are asynchronous on macOS, so exit only once the line is written.
  const exit = () => app.exit(passed ? 0 : 1);
  setTimeout(exit, 2000);
  process.stdout.write(`FIXTURE_RESULT ${JSON.stringify(result)}\n`, exit);
}

function exitOf(pid) {
  const found = events.find((e) => e.kind === "exit" && e.pid === pid);
  if (found) return Promise.resolve(found);
  let timer;
  return new Promise((resolve, reject) => {
    waiters.push({ pid, resolve });
    timer = setTimeout(
      () => reject(new Error(`utility ${pid} did not exit within ${EXIT_WITHIN_MS} ms`)),
      EXIT_WITHIN_MS,
    );
  }).finally(() => clearTimeout(timer));
}

function onUtility(event) {
  events.push(event);
  if (event.kind === "spawn") {
    result.pids.push(event.pid);
    pidOfPage[page] ??= event.pid;
  }
  if (event.kind !== "exit") return;
  for (const w of waiters.splice(0)) {
    if (w.pid === event.pid) w.resolve(event);
    else waiters.push(w);
  }
}

ipcMain.handle("fixture:page", () => page);
ipcMain.handle("fixture:utility-pid", (_e, n) => pidOfPage[n]);
ipcMain.handle("fixture:exit-of", async (_e, pid) => ({ code: (await exitOf(pid)).code }));
ipcMain.handle("fixture:kill-utility", () => {
  const pid = pidOfPage[page];
  process.kill(pid, "SIGKILL");
  return pid;
});

ipcMain.on("fixture:done", async (_e, r) => {
  result.ran += r.ran;
  result.failures.push(...r.failures);
  if (r.startupMs !== undefined) result.startupMs.push(r.startupMs);
  if (r.failures.length > 0 || r.next === undefined) return finish();
  if (r.next === "reload") {
    page += 1;
    win.webContents.reload();
    return;
  }
  // A closed window must not leave its utility running.
  const exited = exitOf(pidOfPage[page]);
  win.close();
  try {
    await exited;
  } catch (e) {
    result.failures.push(`closing the window: ${e.message}`);
  }
  finish();
});

setTimeout(() => finish("watchdog"), WATCHDOG_MS);
app.disableHardwareAcceleration();
// The lifecycle script closes the window; that must not quit the run.
app.on("window-all-closed", () => {});

app
  .whenReady()
  .then(async () => {
    // The CJS build, as an unbundled CJS main loads it.
    const { serve } = require("@ubjs/electron/main");
    // An app that also loads the ESM build must get the same handler.
    const esm = await import("@ubjs/electron/main");
    if (esm.serve !== serve) result.failures.push("the ESM and CJS builds of /main made two instances");

    session.defaultSession.registerPreloadScript({
      type: "frame",
      filePath: require.resolve("@ubjs/electron/preload"),
    });
    win = new BrowserWindow({
      show: false,
      webPreferences: {
        preload: path.join(__dirname, "preload.cjs"),
        // A hidden window's timers must keep time.
        backgroundThrottling: false,
      },
    });
    const wc = win.webContents;
    wc.on("console-message", (e) => console.log(`[page:${e.level}] ${e.message}`));
    // Before the page loads, so its first connect() is served.
    serve(wc, {
      entry: path.join(OUT, "utility.cjs"),
      library: process.env.FIXTURE_LIBRARY,
      info: () => ({ dataDir: app.getPath("userData") }),
      onUtility,
    });
    await win.loadFile(path.join(OUT, "index.html"));
  })
  .catch((e) => finish(`main: ${e.stack}`));
