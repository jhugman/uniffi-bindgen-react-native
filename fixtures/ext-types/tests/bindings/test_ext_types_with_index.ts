/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "./ext_types_index_checks";
import { asyncTest } from "@/asserts";
import { uniffiInitAsync } from "@/generated";

// Sync player flavors need no asset to initialize. Avoid top-level await
// because the JSI bundle targets es5.
asyncTest("uniffiInitAsync is idempotent", async (t) => {
  await uniffiInitAsync();
  await uniffiInitAsync();
  t.end();
});
