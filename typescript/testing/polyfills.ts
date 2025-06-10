/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "abortcontroller-polyfill/dist/abortcontroller-polyfill-only";
import { Console as HermesConsole, URL as HermesURL } from "./hermes";

export type RuntimeContext = "nodejs" | "hermes" | "browser";

export function __runtimeContext(): RuntimeContext {
  if ((globalThis as any).print !== undefined) {
    return "hermes";
  }
  if ((globalThis as any).document !== undefined) {
    return "browser";
  }
  return "nodejs";
}

if (globalThis.console === undefined) {
  (globalThis as any).console = new HermesConsole();
}
if (globalThis.URL === undefined) {
  (globalThis as any).URL = HermesURL;
}

export default globalThis;
