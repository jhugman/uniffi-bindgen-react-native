/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { stringify } from "./hermes";
import "./polyfills";

export class AssertError extends Error {
  constructor(message: string, error?: Error) {
    super(`${message}${error ? `: ${error.message}` : ""}`);
    if (error) {
      this.stack = this.stack
        ? [this.stack, "Caused by:", error.stack].join("\n")
        : error.stack;
    }
  }
}

function isEqual<T>(a: T, b: T): boolean {
  return a === b || a == b || stringify(a) === stringify(b);
}

function checkThrown(
  t: Asserts,
  errorVariant: (err: any) => boolean,
  e: any | undefined,
) {
  if (e === undefined) {
    t.fail("No error was thrown");
  } else if (e instanceof Error) {
    // Good, success!
    if (typeof errorVariant === "string") {
      t.assertEqual(
        getErrorName(e),
        errorVariant,
        "Error is thrown, but the wrong one",
      );
    } else {
      t.assertTrue(errorVariant(e), `Error is thrown, but the wrong one: ${e}`);
    }
  } else {
    t.fail(`Something else was thrown: ${e}`);
  }
}

function getErrorName(error: Error): string {
  const typeName = (error as any)._uniffiTypeName ?? "Error";
  const variantName = (error as any)._uniffiVariantName ?? "unknown";
  return `${typeName}.${variantName}`;
}

// All the assert methods in a single class
export class Asserts {
  fail(message?: string, error?: Error): never {
    throw new AssertError(message ?? "Assertion failed", error);
  }
  assertTrue(condition: boolean, message?: string): void {
    if (condition) {
      return;
    }
    this.fail(message ?? "Expected true, was false");
  }
  assertFalse(condition: boolean, message?: string): void {
    this.assertTrue(!condition, message ?? "Expected false, was true");
  }
  assertNotNull(thing: any | undefined | null, message?: string): void {
    const m = message ?? "Expected to be defined, but was";
    this.assertTrue(thing !== undefined && thing !== null, `${m}: ${thing}`);
  }
  assertNull(thing: any | undefined | null, message?: string): void {
    const m = message ?? "Expected to be null or undefined, but was";
    this.assertTrue(thing === undefined || thing === null, `${m}: ${thing}`);
  }
  assertEqual<T>(
    left: T,
    right: T,
    message?: string,
    equality: (a: T, b: T) => boolean = isEqual,
  ): void {
    const m = message ?? "Expected left and right to be equal";
    this.assertTrue(
      equality(left, right),
      `${m}: ${stringify(left)} !== ${stringify(right)}`,
    );
  }
  assertNotEqual<T>(
    left: T,
    right: T,
    message?: string,
    equality: (a: T, b: T) => boolean = isEqual,
  ): void {
    const m = message ?? "Expected left and right to not be equal";
    this.assertFalse(
      equality(left, right),
      `${m}: ${stringify(left)} === ${stringify(right)}`,
    );
  }
  assertInRange<T = number | bigint>(
    left: T,
    min: T,
    max: T,
    message?: string,
  ): void {
    const m = message ?? "Not in range";
    this.assertTrue(
      min <= left && left <= max,
      `${m}: ${min} <= ${left} <= ${max}`,
    );
  }

  /// We can't use instanceof here: hermes does not seem to generate the right
  /// prototype chain, so we'll check the error message instead.
  assertThrows<T>(errorVariant: (err: any) => boolean, fn: () => T): void {
    let error: any | undefined;
    try {
      fn();
    } catch (e: any) {
      error = e;
    }
    checkThrown(this, errorVariant, error);
  }

  measure<T>(fn: () => T, minMs: number, maxMs?: number, message?: string): T {
    const m = message ?? "Duration out of range";
    const [min, max] = range(minMs, maxMs);
    const start = Date.now();
    const result = fn();
    const end = Date.now();
    const duration = end - start;
    this.assertInRange(duration, min, max, m);
    return result;
  }
}

// Additional methods for running async tests.

function range(expectedMs: number, tolerance?: number): [number, number] {
  if (tolerance === undefined) {
    tolerance = 10;
    return [expectedMs - tolerance, expectedMs + tolerance];
  } else if (tolerance < expectedMs) {
    return [expectedMs - tolerance, expectedMs + tolerance];
  } else {
    // the second arg is greater than the first; treat it like a min/max.
    return [expectedMs, tolerance];
  }
}

export class AsyncAsserts extends Asserts {
  protected timerPromise: Promise<void>;
  private timerResolve: (value: unknown) => void;
  constructor(testName: string, timeout: number) {
    super();
    let timerId = setTimeout(() => {
      this.fail(`Test '${testName}' timed out`);
    }, timeout) as unknown as string | number;
    let timerResolve: (value: unknown) => void;
    this.timerPromise = new Promise((resolve, reject) => {
      timerResolve = resolve;
    }).then(() => {
      clearTimeout(timerId);
    });

    this.timerResolve = timerResolve!;
  }

  async assertThrowsAsync<T>(
    errorVariant: (err: any) => boolean,
    fn: () => Promise<T>,
  ): Promise<void> {
    let error: any | undefined;
    try {
      await fn();
    } catch (e: any) {
      error = e;
    }
    checkThrown(this, errorVariant, error);
    return Promise.resolve();
  }

  async asyncMeasure<T>(
    fn: () => Promise<T>,
    minMs: number,
    maxMs?: number,
    message?: string,
  ): Promise<T> {
    const m = message ?? "Duration out of range";
    const [min, max] = range(minMs, maxMs);
    const start = Date.now();
    const result = await fn();
    const end = Date.now();
    const duration = end - start;
    this.assertInRange(duration, min, max, m);
    return result;
  }

  end() {
    this.timerResolve(0);
  }
}

// For running the tests themselves.
export function test<T>(testName: string, testBlock: (t: Asserts) => T): T {
  try {
    return testBlock(new Asserts());
  } catch (e) {
    console.error(testName, e);
    throw e;
  }
}

export function xtest<T>(
  testName: string,
  testBlock?: (t: Asserts) => T,
): void {
  console.log(`Skipping: ${testName}`);
}

export async function xasyncTest<T>(
  testName: string,
  testBlock: (t: AsyncAsserts) => Promise<T>,
  timeout: number = 1000,
): Promise<T | void> {
  Promise.resolve(xtest(testName));
}

export async function asyncTest<T>(
  testName: string,
  testBlock: (t: AsyncAsserts) => Promise<T>,
  timeout: number = 10000,
): Promise<T> {
  try {
    let asserts = new AsyncAsserts(testName, timeout);
    let v = await testBlock(asserts);
    (await (asserts as any).timerPromise) as Promise<void>;
    return v;
  } catch (e) {
    console.error(testName, e);
    throw e;
  }
}
