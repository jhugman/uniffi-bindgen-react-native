/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { ModuleDefinitions } from "@ubjs/core";
import { serveUtilityWith, type ServeUtilityOptions } from "./core.js";

let serving = false;

/** Serves `definitions` (the generated player-definitions) on the port main
 * hands over. Call it once, synchronously, at the top of the utility's entry.
 * The process exits 0 when the page's port closes, and 1 if it cannot start:
 * a bad init, a throwing onInit, an @ubjs/node that fails to load, or a
 * library that fails to open or register. */
export function serveUtility(
  definitions: ModuleDefinitions,
  options?: ServeUtilityOptions,
): void {
  const { parentPort } = process;
  if (!parentPort) {
    throw new Error("serveUtility must run in an Electron utility process");
  }
  // A second call would open the library again and answer every call twice.
  if (serving) throw new Error("serveUtility() was already called");
  serving = true;
  serveUtilityWith(
    {
      parentPort,
      exit: (code) => process.exit(code),
      // Loaded on init, so an addon that fails to load is reported to the
      // page like a library that fails to open.
      open: async (library) => {
        const { UniffiNativeModule } = await import("@ubjs/node");
        return UniffiNativeModule.open(library);
      },
    },
    definitions,
    options,
  );
}

export { fromMessagePortMain } from "./port.js";
export type { UtilityInit, ServeUtilityOptions } from "./core.js";
