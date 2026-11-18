/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ServeOptions } from "./core.js";

// A whole path segment, on either separator, so `my-app.asar` and `app.asar.unpacked` stay.
const ASAR_SEGMENT = /(^|[\\/])app\.asar(?=[\\/]|$)/;

/** A utility cannot dlopen from inside the archive, so a library path into
 * `app.asar` points at its copy in `app.asar.unpacked`. */
export function unpackedPath(path: string): string {
  return path.replace(ASAR_SEGMENT, "$1app.asar.unpacked");
}

/** Port 0 lets each utility pick a free port, so several can be debugged at once. */
export function inspectArgv(
  inspect: ServeOptions["inspect"],
  isPackaged: boolean,
  warn: (message: string) => void,
): string[] {
  if (!inspect) return [];
  if (isPackaged) {
    warn("serve() ignores inspect in a packaged app");
    return [];
  }
  return ["--inspect=0"];
}
