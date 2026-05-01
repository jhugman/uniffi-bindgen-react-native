/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-example-callbacks-deadlock-regression -- jsi
//   cargo test -p uniffi-example-callbacks-deadlock-regression -- wasm

import generated, {
  callDelete,
  EventSource,
  EventListener,
  KeywordListener,
} from "@/generated/uniffi_callbacks";
import { asyncTest, AsyncAsserts, test } from "@/asserts";
import "@/polyfills";

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

class KeywordListenerImpl implements KeywordListener {
  delete_(value: string): string {
    console.log("delete_", value);
    return `deleted:${value}`;
  }
}

async function testToMax(max: number, t: AsyncAsserts) {
  const listener = new EventListenerImpl(t, max);
  const source = new EventSource(listener);
  source.start(`Going to ${max}, now at:`, max);
}

test("Reserved C++ keyword callback methods work end-to-end", (t) => {
  const listener = new KeywordListenerImpl();
  t.assertEqual(callDelete(listener, "abc"), "deleted:abc");
});

(async () => {
  for (let i = 1; i <= 4096; i *= 2) {
    await asyncTest(
      `Full tilt test up to ${i}`,
      async (t) => await testToMax(i, t),
    );
  }
})();
