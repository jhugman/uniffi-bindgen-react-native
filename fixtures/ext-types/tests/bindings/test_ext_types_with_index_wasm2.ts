/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import "./ext_types_index_checks";
import { asyncTest } from "@/asserts";
import { uniffiInitAsync } from "@/generated";

asyncTest("uniffiInitAsync is idempotent", async (t) => {
  const source = new URL(
    "../../generated/wasm2/ts/uniffi_ext_types_lib.wasm",
    import.meta.url,
  );
  const first = uniffiInitAsync(source);
  const second = uniffiInitAsync(source);
  t.assertTrue(first === second);
  await first;
  await second;
  t.end();
});
