/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Locate a compiled cdylib by walking up from the test directory to find the
// workspace root's target/debug/ directory. This works whether the fixture
// crate has its own workspace or is a member of a parent workspace.

import { existsSync } from "node:fs";
import { join, dirname } from "node:path";

function findTargetDir(startDir) {
  let dir = startDir;
  while (dir !== dirname(dir)) {
    const candidate = join(dir, "target", "debug");
    if (existsSync(candidate)) return candidate;
    dir = dirname(dir);
  }
  throw new Error(`Could not find target/debug/ above ${startDir}`);
}

const targetDebug = findTargetDir(import.meta.dirname);

export function libPath(libName) {
  const fileName =
    process.platform === "darwin" ? `lib${libName}.dylib` : `lib${libName}.so`;
  return join(targetDebug, fileName);
}
