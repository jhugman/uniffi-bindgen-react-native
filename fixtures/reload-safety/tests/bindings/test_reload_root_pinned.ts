/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// The root that disarms a module on teardown is reachable from JS only through
// the `uniffi` global, and app code can delete or overwrite that. Every module
// object holds its root, so a module JS can still call is a module still armed.
//
// Without that hold the root is collected while JS still holds the module: the
// module is disarmed underneath it, so calls into it fail and its callbacks
// return the zeroed result core writes.
//
// To run:
//   cargo test -p uniffi-fixture-reload-safety -- jsi2::test_reload_root_pinned
import { test } from "@/asserts";
import theModule, {
  type Greeter,
  invokeStashedGreeter,
  stashGreeter,
} from "@/generated/reload_safety";

theModule.initialize();

const greeter: Greeter = {
  greet: () => "hello",
};

test("a live module keeps its player root alive", (t) => {
  stashGreeter(greeter);
  t.assertEqual(
    invokeStashedGreeter(),
    "hello",
    "the greeter answers before anything is collected",
  );

  // The generated wrapper keeps the module object and drops everything else
  // `open` handed it, so after this the module object is the only holder left.
  delete (globalThis as any).uniffi;
  (globalThis as any).__hermesGc();

  t.assertEqual(
    invokeStashedGreeter(),
    "hello",
    "collecting the uniffi global disarmed a module that is still callable",
  );
});
