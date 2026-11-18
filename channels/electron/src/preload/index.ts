/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Bundled into dist/preload.cjs. A sandboxed preload can require only electron.
import { contextBridge, ipcRenderer } from "electron";
import { installPreload } from "./core.js";

installPreload({
  ipcRenderer,
  contextBridge,
  window,
  contextIsolated: process.contextIsolated,
});
