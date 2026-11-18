/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Bundles one fixture script for a sandboxed Electron page and runs it. Then
// checks what only a runner can see: main's exit, the page bundle, the
// utilities' output, and that no utility outlives the run.
//
//   node runner.mjs <out dir> <script> <generated ts dir> <cdylib> [--typecheck]
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const [out, script, generated, library, flag] = process.argv.slice(2);
const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const pkg = path.join(repo, "channels/electron");
const require = createRequire(path.join(pkg, "package.json"));
const esbuild = require("esbuild");
const electron = require("electron");

const KILL_AFTER_MS = 150_000;
const GRACE_MS = 5000;
// Request to init. CI catches only a hang; an app wants about 200 ms.
const STARTUP_BOUND_MS = 10_000;
// A page behind a port must bundle no Node code and no player.
const FORBIDDEN = /node_modules\/(electron|@ubjs\/node|@ubjs\/wasm)\/|runtimes\/(napi|wasm)\/|^node:/;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const failures = [];

function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** A negative pid is a process group. */
function kill(pid) {
  try {
    process.kill(pid, "SIGKILL");
  } catch (e) {
    // Gone already; macOS answers EPERM for a group of zombies.
    if (e.code !== "ESRCH" && !(pid < 0 && e.code === "EPERM")) throw e;
  }
}

mkdirSync(out, { recursive: true });
for (const f of ["main.cjs", "preload.cjs", "index.html"]) {
  copyFileSync(path.join(here, f), path.join(out, f));
}
// Sources, not dist: one copy of each package in a bundle, as an app gets.
const tsconfig = path.join(out, "tsconfig.json");
writeFileSync(
  tsconfig,
  JSON.stringify({
    compilerOptions: {
      target: "ES2022",
      module: "ESNext",
      moduleResolution: "Bundler",
      strict: true,
      noEmit: true,
      skipLibCheck: true,
      allowImportingTsExtensions: true,
      lib: ["ES2022", "DOM"],
      types: ["node"],
      paths: {
        "@/asserts": [path.join(here, "asserts.ts")],
        "@/generated/*": [path.join(generated, "*")],
        "@/*": [path.join(repo, "typescript/testing/*")],
        "@fixture/script": [script],
        "@ubjs/core": [path.join(repo, "typescript/src/index.ts")],
        "@ubjs/worker": [path.join(repo, "channels/worker/src/index.ts")],
        "@ubjs/electron/page": [path.join(pkg, "src/page/index.ts")],
        "@ubjs/electron/utility": [path.join(pkg, "src/utility/index.ts")],
      },
    },
    files: [path.join(here, "app-types.ts")],
  }),
);

const page = await esbuild.build({
  entryPoints: [path.join(here, "bootstrap.ts")],
  bundle: true,
  format: "esm",
  platform: "browser",
  outfile: path.join(out, "page.js"),
  tsconfig,
  metafile: true,
  logLevel: "error",
});
for (const input of Object.keys(page.metafile.inputs)) {
  if (FORBIDDEN.test(input)) failures.push(`the page bundles ${input}`);
}
await esbuild.build({
  entryPoints: [path.join(here, "utility.ts")],
  bundle: true,
  format: "cjs",
  platform: "node",
  outfile: path.join(out, "utility.cjs"),
  tsconfig,
  external: ["electron", "@ubjs/node"],
  logLevel: "error",
});
if (flag === "--typecheck") {
  const tsc = spawnSync(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", tsconfig], {
    encoding: "utf8",
  });
  if (tsc.status !== 0) failures.push(`an app's calls do not type-check:\n${tsc.stdout}${tsc.stderr}`);
}

const env = { ...process.env, FIXTURE_OUT: out, FIXTURE_LIBRARY: library };
delete env.ELECTRON_RUN_AS_NODE;
// In its own process group, so a hang is killed with every utility.
const child = spawn(electron, [path.join(out, "main.cjs")], {
  detached: true,
  stdio: ["ignore", "pipe", "pipe"],
  env,
});
/** Echoes a stream and keeps its lines. Utilities inherit main's stdout and
 * stderr, so the pipe can outlive main. */
function capture(stream, echo) {
  const lines = [];
  const reader = createInterface({ input: stream });
  reader.on("line", (line) => {
    echo(line);
    lines.push(line);
  });
  return { lines, ended: new Promise((resolve) => reader.once("close", resolve)) };
}
const stdout = capture(child.stdout, (l) => console.log(l));
const stderr = capture(child.stderr, (l) => console.error(l));

const killTimer = setTimeout(() => {
  failures.push(`Electron was killed after ${KILL_AFTER_MS} ms`);
  kill(-child.pid);
}, KILL_AFTER_MS);
const exit = await new Promise((resolve, reject) => {
  child.once("error", reject);
  child.once("exit", (code, signal) => resolve({ code, signal }));
});
clearTimeout(killTimer);
await Promise.race([Promise.all([stdout.ended, stderr.ended]), sleep(GRACE_MS)]);

const line = stdout.lines.findLast((l) => l.startsWith("FIXTURE_RESULT "));
const result = line ? JSON.parse(line.slice("FIXTURE_RESULT ".length)) : undefined;
if (!result) {
  failures.push(`Electron printed no result, and exited with ${exit.code ?? exit.signal}`);
} else {
  failures.push(...result.failures);
  if (result.ran === 0) failures.push("the script ran no tests");
  if (exit.code !== 0 && result.failures.length === 0) failures.push(`Electron exited with ${exit.code}`);
  for (const ms of result.startupMs) {
    if (!(ms <= STARTUP_BOUND_MS)) failures.push(`startup took ${ms} ms`);
  }
  // Main must not swallow a utility's output: each prints these on init.
  for (const pid of result.pids) {
    if (!stdout.lines.some((l) => l.trim() === `[utility] init ${pid}`)) failures.push(`no stdout from utility ${pid}`);
    if (!stderr.lines.some((l) => l.trim() === `[utility] stderr ${pid}`)) failures.push(`no stderr from utility ${pid}`);
  }
  // A utility ends a few ms after main, and a zombie answers kill(pid, 0) until it is reaped.
  const deadline = Date.now() + GRACE_MS;
  let alive = result.pids.filter(isAlive);
  while (alive.length > 0 && Date.now() < deadline) {
    await sleep(100);
    alive = alive.filter(isAlive);
  }
  for (const pid of alive) {
    failures.push(`utility ${pid} outlived the run`);
    kill(pid);
  }
  console.log(`\n${result.ran} tests; startup ${result.startupMs.join(", ")} ms`);
}
kill(-child.pid);

for (const f of failures) console.error(`FAIL: ${f}`);
process.exit(failures.length === 0 ? 0 : 1);
