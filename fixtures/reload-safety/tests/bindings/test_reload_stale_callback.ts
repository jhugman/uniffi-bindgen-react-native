/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// A callback whose JS function belongs to a runtime that no longer exists.
//
// Iteration 1 hands Rust a Greeter and Rust stashes it process-globally, so it
// outlives the runtime that created it. Iteration 2 — a fresh runtime, the same
// loaded library — asks Rust to call it. The trampoline still holds a
// `jsi::Runtime *` to freed memory and a `jsi::Function` from a dead heap; only
// the unloading flag stops it being used. Disarmed, the call returns the zeroed
// result core writes, which lifts as an empty string.
//
// To run:
//   cargo test -p uniffi-fixture-reload-safety -- jsi2::test_reload_stale_callback
import { test } from "@/asserts";
import theModule, {
  type Greeter,
  invokeStashedGreeter,
  noteIteration,
  stashGreeter,
} from "@/generated/reload_safety";

const greeter: Greeter = {
  greet: () => "hello",
};

test("a callback from a destroyed runtime returns instead of crashing", (t) => {
  const iteration = noteIteration();

  if (iteration > 1) {
    // The line that crosses the reload boundary, and it runs before
    // initialize(): registering this runtime's vtable overwrites the pointer
    // cell Rust dispatches through, after which the stashed greeter reaches
    // this runtime's live trampolines and the stale ones are unreachable. The
    // gap between one runtime dying and the next one registering is the window
    // a Rust thread can call into, so it is the window under test.
    t.assertEqual(
      invokeStashedGreeter(),
      "",
      "iteration 1's greeter was called from a destroyed runtime and answered " +
        "with something: a disarmed trampoline should have returned nothing",
    );
  }

  // Registers this runtime's Greeter vtable.
  theModule.initialize();

  if (iteration === 1) {
    t.assertNull(
      invokeStashedGreeter(),
      "nothing is stashed before the first runtime stashes it",
    );
    stashGreeter(greeter);
    t.assertEqual(
      invokeStashedGreeter(),
      "hello",
      "the greeter answers while its own runtime is alive",
    );
    return;
  }

  // The disarm is scoped to the module of the runtime that went away — this
  // runtime's own callbacks still work.
  stashGreeter(greeter);
  t.assertEqual(
    invokeStashedGreeter(),
    "hello",
    "the new runtime's own greeter was disarmed along with the old one",
  );
});
