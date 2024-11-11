/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/*
fixture=fixtures/callbacks-regression
cargo xtask run \
    --crate $fixture \
    --ts-dir $fixture/generated \
    --cpp-dir $fixture/generated/cpp \
    $fixture/tests/bindings/test_callbacks_regression.ts
*/

import generated, {
  EventSource,
  EventListener,
} from "../../generated/uniffi_callbacks";
import { asyncTest, AsyncAsserts } from "@/asserts";
import { console } from "@/hermes";

// This is only needed in tests.
generated.initialize();

class EventListenerImpl implements EventListener {
  constructor(
    private t: AsyncAsserts,
    private max: number,
  ) {}
  onEvent(message: string, number: number): void {
    // console.log("--", message, number);
    if (number === this.max - 1) {
      console.log("-- Done!", this.max);
      this.t.end();
    }
  }
}

async function testToMax(max: number, t: AsyncAsserts) {
  const listener = new EventListenerImpl(t, max);
  const source = new EventSource(listener);
  source.start(`Going to ${max}, now at:`, max);
}

(async () => {
  for (let i = 1; i <= 1024; i *= 2) {
    await asyncTest(
      `Full tilt test up to ${i}`,
      async (t) => await testToMax(i, t),
    );
  }
})();
