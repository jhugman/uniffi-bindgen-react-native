/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PUBLISH_FIELDS = [
  "name",
  "version",
  "description",
  "main",
  "types",
  "module",
  "exports",
  "homepage",
  "repository",
  "license",
  "author",
  "keywords",
  "bugs",
  "publishConfig",
];

const COPY_FILES = ["index.js", "lib.js", "index.d.ts", "README.md"];

const pkgDir = dirname(dirname(fileURLToPath(import.meta.url)));
const rootDir = join(pkgDir, "npm", "root");

rmSync(rootDir, { recursive: true, force: true });
mkdirSync(rootDir, { recursive: true });

for (const f of COPY_FILES) {
  copyFileSync(join(pkgDir, f), join(rootDir, f));
}

const src = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
const workspaceRoot = JSON.parse(
  readFileSync(join(pkgDir, "..", "..", "package.json"), "utf8"),
);
const VERSION = workspaceRoot.version;

const platforms = readdirSync(join(pkgDir, "npm")).filter((d) => d !== "root");
if (platforms.length === 0) {
  console.error("No npm/<triple>/ dirs found — run `napi create-npm-dir` first.");
  process.exit(1);
}

for (const p of platforms) {
  const ppath = join(pkgDir, "npm", p, "package.json");
  const ppkg = JSON.parse(readFileSync(ppath, "utf8"));
  ppkg.version = VERSION;
  writeFileSync(ppath, JSON.stringify(ppkg, null, 2) + "\n");
}

const rootPkg = Object.fromEntries(
  PUBLISH_FIELDS.filter((k) => src[k] !== undefined).map((k) => [k, src[k]]),
);
rootPkg.version = VERSION;
rootPkg.optionalDependencies = Object.fromEntries(
  platforms.map((p) => [`${src.name}-${p}`, VERSION]),
);

writeFileSync(
  join(rootDir, "package.json"),
  JSON.stringify(rootPkg, null, 2) + "\n",
);

console.log(
  `Assembled ${rootDir} @ ${VERSION} with ${platforms.length} optional deps (${platforms.join(", ")}).`,
);
