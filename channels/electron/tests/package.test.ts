/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import assert from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (rel: string) =>
  JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));
const pkg = read("../package.json");
const root = read("../../../package.json");

test("the version is the root's, and every @ubjs range is ^ plus it", () => {
  assert.strictEqual(pkg.version, root.version);
  for (const field of ["dependencies", "peerDependencies"])
    for (const [name, range] of Object.entries(pkg[field]))
      if (name.startsWith("@ubjs/"))
        assert.strictEqual(range, `^${pkg.version}`, `${field}.${name}`);
  assert.deepStrictEqual(Object.keys(pkg.dependencies).sort(), [
    "@ubjs/node",
    "@ubjs/worker",
  ]);
  assert.ok("@ubjs/core" in pkg.peerDependencies);
});

test("electron is an optional peer from 41, developed against 41.10.2", () => {
  assert.strictEqual(pkg.peerDependencies.electron, ">=41");
  assert.deepStrictEqual(pkg.peerDependenciesMeta, {
    electron: { optional: true },
  });
  assert.strictEqual(pkg.devDependencies.electron, "41.10.2");
});

// Only /main: an unbundled CJS main may resolve as node10. /utility and /page
// are ESM for bundlers, and their types import @ubjs/worker, which node10 cannot find.
test("typesVersions gives node10 /main's CJS types", () => {
  const strip = (p: string) => p.replace(/^\.\//, "");
  assert.deepStrictEqual(pkg.typesVersions, {
    "*": { main: [strip(pkg.exports["./main"].require.types)] },
  });
});
