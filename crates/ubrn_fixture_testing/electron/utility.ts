/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The utility entry, as an app writes its own.
import { serveUtility } from "@ubjs/electron/utility";
import { PLAYER_DEFINITIONS } from "@/generated/player-definitions";

// The runner looks for these lines: a utility's stdout and stderr reach main's.
serveUtility(PLAYER_DEFINITIONS, {
  onInit: () => {
    console.log("[utility] init", process.pid);
    console.error("[utility] stderr", process.pid);
  },
});
