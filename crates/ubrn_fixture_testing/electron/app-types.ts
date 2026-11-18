/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Type-checked, never run: the calls an app makes across packages must compile.
import { connect } from "@ubjs/electron/page";
import { serveUtility } from "@ubjs/electron/utility";
import { uniffiInitAsync } from "@/generated/index.port";
import { PLAYER_DEFINITIONS } from "@/generated/player-definitions";

export async function page(): Promise<void> {
  const conn = await connect();
  await uniffiInitAsync(conn.port);
}

export function utility(): void {
  serveUtility(PLAYER_DEFINITIONS);
}
