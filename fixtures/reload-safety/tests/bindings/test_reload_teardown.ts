/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The player's only teardown hook is the destructor of the `uniffi` host
// object, which JSI runs when the runtime that owns it is destroyed. The
// test-runner evaluates every script against two runtimes in one process, so
// iteration 2 is a full reload and is the only place iteration 1's teardown is
// observable.
//
// To run:
//   cargo test -p uniffi-fixture-reload-safety -- jsi2::test_reload_teardown
import { test } from "@/asserts";
import { noteIteration } from "@/generated/reload_safety";

const teardownCount = (): number =>
  (globalThis as any).uniffi.$teardownCount as number;

test("the player root is destroyed with its runtime", (t) => {
  // Also forces the module to register, so the root has something to hold.
  const iteration = noteIteration();

  if (iteration === 1) {
    t.assertEqual(
      teardownCount(),
      0,
      "no runtime has ended yet in this process",
    );
    return;
  }

  t.assertEqual(
    teardownCount(),
    1,
    "the first runtime's uniffi root was never destroyed, so the player has " +
      "no teardown trigger and nothing is disarmed on reload",
  );
});
