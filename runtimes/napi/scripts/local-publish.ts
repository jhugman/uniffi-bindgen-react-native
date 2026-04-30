/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { execSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const TARGET_FLAGS: Record<string, string> = {
  "x86_64-apple-darwin": "",
  "aarch64-apple-darwin": "",
  "x86_64-unknown-linux-gnu": "--zig",
  "aarch64-unknown-linux-gnu": "--zig",
  "x86_64-unknown-linux-musl": "--zig",
  "aarch64-unknown-linux-musl": "--zig",
  "x86_64-pc-windows-msvc": "",
  "aarch64-pc-windows-msvc": "",
};

const pkgDir = dirname(dirname(fileURLToPath(import.meta.url)));
process.chdir(pkgDir);

const { values } = parseArgs({
  options: {
    targets: { type: "string" },
    "skip-build": { type: "boolean" },
  },
});

function run(cmd: string, cwd: string = pkgDir): void {
  console.log(`\n$ ${cmd}`);
  execSync(cmd, { stdio: "inherit", cwd });
}

const targets = values.targets
  ? values.targets
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
  : [];

if (!values["skip-build"]) {
  if (targets.length === 0) {
    run("npm run build");
  } else {
    for (const t of targets) {
      const flags = TARGET_FLAGS[t] ?? "";
      run(`npm run build -- --target ${t} ${flags}`.trim());
    }
  }
}

const artifactsDir = join(pkgDir, "artifacts");
rmSync(artifactsDir, { recursive: true, force: true });
mkdirSync(artifactsDir, { recursive: true });

const built = readdirSync(pkgDir).filter((f) =>
  /^uniffi-runtime-napi\..*\.node$/.test(f),
);
if (built.length === 0) {
  console.error("No uniffi-runtime-napi.*.node files found — build first.");
  process.exit(1);
}
for (const f of built) copyFileSync(join(pkgDir, f), join(artifactsDir, f));
console.log(`\nStaged ${built.length} artifact(s) into ./artifacts/`);

rmSync(join(pkgDir, "npm"), { recursive: true, force: true });
mkdirSync(join(pkgDir, "npm"), { recursive: true });
run("npm run create-npm-dirs");
run("npm run artifacts");
run("npm run assemble-root");

const npmRoot = join(pkgDir, "npm");
const platformDirs = readdirSync(npmRoot).filter((d) => d !== "root");
for (const d of [...platformDirs, "root"]) {
  const dir = join("npm", d);
  console.log(`\n--- ${dir} ---`);
  console.log(readFileSync(join(npmRoot, d, "package.json"), "utf8"));
}

for (const d of platformDirs) {
  run("npm publish --dry-run --access public --tag latest", join(npmRoot, d));
}
run(
  "npm publish --dry-run --access public --tag latest",
  join(npmRoot, "root"),
);

console.log("\nLocal simulation complete — nothing was published.");
