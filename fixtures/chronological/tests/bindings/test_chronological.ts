/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  now as rustNow,
  toStringTimestamp,
  returnDuration,
  returnTimestamp,
  add,
  diff,
  ChronologicalError,
  optional,
} from "../../generated/chronological";
import { asyncTest, test, Asserts, xtest } from "@/asserts";

type Duration = number;

const Duration = {
  ofSeconds(sec: number): Duration {
    return sec * 1000;
  },
  ofNanos(nanos: number): Duration {
    return nanos / 1e6;
  },
  ofMs(ms: number): Duration {
    return ms;
  },
};

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date#the_epoch_timestamps_and_invalid_date
const MAX_MS_FROM_EPOCH = 8.64e15;
const Instant = {
  ofEpochSecond(seconds: number): Date {
    return new Date(seconds * 1000);
  },

  parse(string: string): Date {
    return new Date(Date.parse(string));
  },

  now(): Date {
    return new Date();
  },

  MAX: new Date(MAX_MS_FROM_EPOCH),
};

test("One way timestamp", (t) => {
  {
    const now = new Date(22, 1);
    t.assertEqual(now.toISOString(), toStringTimestamp(now));
  }

  {
    const now = new Date();
    t.assertEqual(now.toISOString(), toStringTimestamp(now));
  }
});

test("Roundtripping Timestamp", (t) => {
  const now = new Date();
  dateEquals(t, now, returnTimestamp(now));
});

test("Roundtripping Duration", (t) => {
  function rt(duration: Duration) {
    t.assertEqual(duration, returnDuration(duration));
  }

  rt(Duration.ofMs(1));
  rt(Duration.ofMs(500));
  rt(Duration.ofMs(1500) + Duration.ofSeconds(1));

  rt(Duration.ofNanos(5e5));
});

function dateEquals(t: Asserts, a: Date, b: Date) {
  t.assertEqual(a.getTime(), b.getTime(), `${a} !== ${b}`);
}

test("Rust vs Hermes timestamp", (t) => {
  const now = new Date();
  now.setMilliseconds(0);
  const rustTime = rustNow();
  rustTime.setMilliseconds(0);

  dateEquals(t, now, rustTime);
});

test("Test passing timestamp and duration while returning timestamp", (t) => {
  const start = Instant.ofEpochSecond(1000);
  const duration = Duration.ofSeconds(2);
  t.assertEqual(add(start, duration), Instant.ofEpochSecond(1000 + 2));
});

test("Test passing timestamp while returning duration", (t) => {
  const start = Instant.ofEpochSecond(1000);
  const end = Instant.ofEpochSecond(1002);
  t.assertEqual(diff(end, start), Duration.ofSeconds(2));
});

test("Test pre-epoch timestamps", (t) => {
  const start = Instant.parse("1955-11-05T00:06:00.283000001Z");
  t.assertEqual(start, add(start, 0));

  const duration = Duration.ofSeconds(1) + Duration.ofNanos(1);
  const end = Instant.parse("1955-11-05T00:06:01.283000002Z");
  t.assertEqual(end, add(start, duration));
});

test("Test exceptions are propagated", (t) => {
  t.assertThrows(ChronologicalError.TimeDiffError.instanceOf, () => {
    diff(Instant.ofEpochSecond(100), Instant.ofEpochSecond(101));
  });
});

test("Test max Instant upper bound", (t) => {
  t.assertEqual(add(Instant.MAX, Duration.ofSeconds(0)), Instant.MAX);
});

test("Test max Instant upper bound overflow", (t) => {
  // NB: Javascript Dates have a smaller overflow than Rust,
  // so this error is internal to uniffi.
  t.assertThrows(
    (e: Error) => e.message.indexOf("Date overflow") >= 0,
    () => {
      add(Instant.MAX, Duration.ofSeconds(1));
    },
  );
});

function delayPromise(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

asyncTest(
  "Test that rust timestamps behave like JS timestamps",
  async (t): Promise<void> => {
    // Unfortunately the JS clock may be lower resolution than the Rust clock.
    // Sleep for 1ms between each call, which should ensure the JVM clock ticks
    // forward.

    const tsBefore = Instant.now();
    await delayPromise(10);
    const rsNow = rustNow();
    await delayPromise(10);

    const tsAfter = Instant.now();
    t.assertTrue(tsBefore < rsNow);
    t.assertTrue(tsAfter > rsNow);
    t.end();
  },
);

test("Test optional values work", (t) => {
  t.assertTrue(optional(Instant.MAX, Duration.ofSeconds(0)));
  t.assertFalse(optional(undefined, Duration.ofSeconds(0)));
  t.assertFalse(optional(Instant.MAX, undefined));
});
