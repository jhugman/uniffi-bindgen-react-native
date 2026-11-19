/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { existsSync } from "node:fs";
import { app, ipcMain, MessageChannelMain, utilityProcess } from "electron";
import { createMain, sharedMain, type MainApi } from "./core.js";

// In plain Node, require("electron") is a path, and a utility or a preload has
// no ipcMain. The ESM build fails sooner there, when it links.
if (typeof ipcMain?.on !== "function") {
  throw new Error("@ubjs/electron/main must run in the Electron main process");
}

// Installs the connect handler on import.
const main = sharedMain(() =>
  createMain({
    ipcMain,
    utilityProcess,
    MessageChannelMain,
    isPackaged: () => app.isPackaged,
    fileExists: existsSync,
    warn: (message) => console.warn(`@ubjs/electron: ${message}`),
  }),
);

/** Serves the library to the page in `wc`: when the page calls connect(), main
 * forks a utility on `options.entry`, and hands the page its port once the
 * library is open. Call it before the window loads. `close()` stops serving. */
export const serve: MainApi["serve"] = main.serve;

export type { ServeOptions, UtilityEvent } from "./core.js";
