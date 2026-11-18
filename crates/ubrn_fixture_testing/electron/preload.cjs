/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Test-only: a sandboxed page cannot exit or write to stdout, so it reports to main.
const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("__fixture", {
  sandboxed: process.sandboxed === true,
  contextIsolated: process.contextIsolated === true,
  page: () => ipcRenderer.invoke("fixture:page"),
  utilityPid: (page) => ipcRenderer.invoke("fixture:utility-pid", page),
  exitOf: (pid) => ipcRenderer.invoke("fixture:exit-of", pid),
  killUtility: () => ipcRenderer.invoke("fixture:kill-utility"),
  done: (result) => ipcRenderer.send("fixture:done", result),
});
