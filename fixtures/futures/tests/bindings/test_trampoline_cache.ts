/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Proves a player bridge builds one trampoline per callback type, not one per
// call.
//
// A trampoline is permanently leaked by design — the library may invoke the
// pointer from any thread long after the call that produced it — so the only
// thing keeping that bounded is reuse. `rust_future_poll_*` passes its
// continuation on every poll, so without reuse an await leaks a CbUserData and
// a libffi closure per poll: measured at ~720 bytes, reaching 295 MB RSS over
// 400k polls and still climbing linearly.
//
// The count is asserted rather than the memory. RSS is too noisy to assert on,
// and nothing else observable moves: the continuation is a module-level const,
// so the leak pins the same JS function object every time and neither the
// Hermes heap nor any allocator count changes. `$uniffiTrampolineCount` is a
// diagnostic both player bridges expose for exactly this reason, counting
// builds at core's single build point.
//
// To run:
//   cargo test -p uniffi-fixture-futures -- jsi2::test_trampoline_cache
//   cargo test -p uniffi-fixture-futures -- napi::test_trampoline_cache
import { alwaysReady } from "@/generated/futures";
import nativeModule from "@/generated/futures-ffi";
import { asyncTest } from "@/asserts";
import "@/polyfills";

// Enough polls that a per-call leak is unambiguous, but quick enough to sit in
// the normal suite — a regression shows up as +1 per poll, so the assertion is
// exact equality rather than a threshold.
const POLLS = 2000;

const trampolineCount = (): number =>
  (nativeModule() as any).$uniffiTrampolineCount();

(async () => {
  await asyncTest(
    "poll continuation reuses one trampoline across polls",
    async (t) => {
      // One call first, so the continuation's trampoline is already built when
      // the baseline is taken. Otherwise the first poll of the loop below
      // legitimately adds one and the assertion is off by one.
      await alwaysReady();
      const before = trampolineCount();

      for (let i = 0; i < POLLS; i++) {
        await alwaysReady();
      }

      const after = trampolineCount();
      t.assertEqual(
        after,
        before,
        `built ${after - before} extra trampolines across ${POLLS} polls ` +
          `(expected 0: the poll continuation is a module-level const, so ` +
          `every poll should hit the cache)`,
      );
      t.end();
    },
    60000,
  );
})();
