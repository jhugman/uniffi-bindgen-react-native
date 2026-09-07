/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import NativeUniffiPlayer from "./NativeUniffiPlayer";

/** What a generated binding hands to `globalThis.uniffi.open(...)`. */
export type UniffiOpenTarget = string | { name: string } | { path: string };

// A full reload builds a new JS runtime with a fresh module registry, so this
// module is evaluated again there and installs again, by design. Within one
// runtime a second import is a no-op.
let installed = false;

export function install(): void {
  if (installed) {
    return;
  }
  if (!NativeUniffiPlayer.install()) {
    throw new Error("@ubjs/react-native: the native player failed to install");
  }
  installed = true;
}

install();
