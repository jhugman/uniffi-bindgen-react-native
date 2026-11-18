/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { build } from "esbuild";

// The CJS build sits under a package.json that says so, since the package itself is ESM.
mkdirSync("dist/cjs", { recursive: true });
writeFileSync(
  "dist/cjs/package.json",
  JSON.stringify({ type: "commonjs" }) + "\n",
);

// A sandboxed preload can require only electron, so the shipped preload is one standalone script.
await build({
  // esbuild keeps only legal comments, so the licence goes in as a banner.
  banner: {
    js: `/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */`,
  },
  entryPoints: ["src/preload/index.ts"],
  bundle: true,
  format: "cjs",
  external: ["electron"],
  target: "es2022",
  outfile: "dist/preload.cjs",
});
