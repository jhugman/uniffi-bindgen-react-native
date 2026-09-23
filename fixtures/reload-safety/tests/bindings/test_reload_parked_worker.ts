/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// A foreign-thread callback parked in the rendezvous when its runtime dies.
//
// cb_dispatch posts the call onto the JS thread and blocks until the task runs.
// A runtime destroyed with that task still queued discards it, so the worker
// waits on a condition variable nothing will ever signal: a Rust thread parked
// forever with a frame of the loaded library below it. Only an abort at
// teardown releases it — the unloading flag alone does not, because the worker
// is already past the point that reads it.
//
// greet() spends a few milliseconds on purpose. The worker is parked for the
// whole of that, so when iteration 1's last timer drains and the runtime is
// destroyed, the worker is parked rather than briefly between calls.
//
// To run:
//   cargo test -p uniffi-fixture-reload-safety -- jsi2::test_reload_parked_worker
import { asyncTest } from "@/asserts";
import theModule, {
  type Greeter,
  noteIteration,
  pingerReturned,
  pingerStarted,
  startForeignThreadPinger,
} from "@/generated/reload_safety";

theModule.initialize();

const BUSY_MS = 3;

let pings = 0;
const pinger: Greeter = {
  greet: () => {
    pings += 1;
    const until = Date.now() + BUSY_MS;
    while (Date.now() < until) {
      // Hold the JS thread, so the worker is parked and not spinning.
    }
    return "pong";
  },
};

const delay = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

(async () => {
  await asyncTest(
    "a parked foreign-thread callback is released at teardown",
    async (t) => {
      const iteration = noteIteration();

      if (iteration === 1) {
        t.assertFalse(pingerStarted(), "nothing has started the pinger yet");
        startForeignThreadPinger(pinger);
        await delay(200);
        t.assertTrue(
          pings > 5,
          `the pinger only reached JS ${pings} times: the cross-thread ` +
            `rendezvous is not running, so this test proves nothing`,
        );
        t.assertFalse(
          pingerReturned(),
          "the pinger stopped while its own runtime was still alive",
        );
        t.end();
        return;
      }

      t.assertTrue(pingerStarted(), "iteration 1 started the pinger");
      // The abort runs on the JS thread during teardown; the worker wakes on
      // another thread, so give it a moment to record that it got out.
      for (let i = 0; i < 40 && !pingerReturned(); i++) {
        await delay(50);
      }
      t.assertTrue(
        pingerReturned(),
        "the worker parked in cb_dispatch never woke: the runtime it was " +
          "waiting on is gone and nothing released the rendezvous",
      );
      t.end();
    },
    10000,
  );
})();
