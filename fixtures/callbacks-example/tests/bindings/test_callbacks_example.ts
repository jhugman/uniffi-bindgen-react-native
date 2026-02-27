/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import generated, {
  CallAnswerer,
  Telephone,
  getSimCards,
  SimCard,
  TelephoneError,
} from "../../generated/callbacks";
import { test } from "@/asserts";

// This is only needed in tests.
generated.initialize();

class SomeOtherError extends Error {}

// Simple example just to see it work.
// Pass in a string, get a string back.
// Pass in nothing, get unit back.
class CallAnswererImpl implements CallAnswerer {
  constructor(private mode: string) {}
  answer(): string {
    if (this.mode === "normal") {
      return "Bonjour";
    } else if (this.mode === "busy") {
      throw new TelephoneError.Busy("I'm busy");
    } else {
      throw new SomeOtherError();
    }
  }
}

test("A Rust sim, with a Typescript call answerer", (t) => {
  const telephone = new Telephone();
  const sim = getSimCards()[0];
  t.assertEqual(telephone.call(sim, new CallAnswererImpl("normal")), "Bonjour");
  telephone.uniffiDestroy();
});

// Our own sim.
class Sim implements SimCard {
  name(): string {
    return "typescript";
  }
}

// Regression test for the multi-vtable clone bug (uniffi 0.30): when two
// foreign-trait vtables (SimCard and CallAnswerer) are registered in the same
// module, the shared CallbackInterfaceClone handler must dispatch to the
// correct per-vtable handleMap. In the buggy version, registering CallAnswerer's
// vtable overwrote the shared CALLBACK thread-local, causing SimCard clones to
// look up handles in CallAnswerer's handleMap and throw a stale-handle error.
test("A typescript sim with a typescript answerer", (t) => {
  const telephone = new Telephone();
  t.assertEqual(
    telephone.call(new Sim(), new CallAnswererImpl("normal")),
    "typescript est bon marché",
  );
  telephone.uniffiDestroy();
});

test("Errors are serialized and returned", (t) => {
  const telephone = new Telephone();
  const sim = getSimCards()[0];
  t.assertThrows(TelephoneError.Busy.instanceOf, () =>
    telephone.call(sim, new CallAnswererImpl("busy")),
  );
  t.assertThrows(TelephoneError.InternalTelephoneError.instanceOf, () =>
    telephone.call(sim, new CallAnswererImpl("something-else")),
  );
  telephone.uniffiDestroy();
});
