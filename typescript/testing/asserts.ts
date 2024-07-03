/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { console, stringify } from "./hermes";

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

export function fail(message?: string, error?: Error): never {
  throw new AssertError(message ?? "Assertion failed", error);
}

export function assertTrue(condition: boolean, message?: string): void {
  if (condition) {
    return;
  }
  fail(message ?? "Expected true, was false");
}

export function assertFalse(condition: boolean, message?: string): void {
  assertTrue(!condition, message ?? "Expected false, was true");
}

export function assertNotNull(
  thing: any | undefined | null,
  message?: string,
): void {
  const m = message ?? "Expected to be defined, but was";
  assertTrue(thing !== undefined && thing !== null, `${m}: ${thing}`);
}

export function assertNull(
  thing: any | undefined | null,
  message?: string,
): void {
  const m = message ?? "Expected to be null or undefined, but was";
  assertTrue(thing === undefined || thing === null, `${m}: ${thing}`);
}

export function assertEqual<T>(
  left: T,
  right: T,
  message?: string,
  equality: (a: T, b: T) => boolean = isEqual,
): void {
  const m = message ?? "Expected left and right to be equal";
  assertTrue(
    equality(left, right),
    `${m}: ${stringify(left)} !== ${stringify(right)}`,
  );
}

export function assertNotEqual<T>(
  left: T,
  right: T,
  message?: string,
  equality: (a: T, b: T) => boolean = isEqual,
): void {
  const m = message ?? "Expected left and right to not be equal";
  assertFalse(
    equality(left, right),
    `${m}: ${stringify(left)} === ${stringify(right)}`,
  );
}

export function test<T>(testName: string, testBlock: () => T): T {
  try {
    return testBlock();
  } catch (e) {
    console.error(testName, e);
    throw e;
  }
}

export function xtest<T>(testName: string, testBlock: () => T): void {
  console.log(`Skipping: ${testName}`);
}

function isEqual<T>(a: T, b: T): boolean {
  return a === b || a == b || stringify(a) === stringify(b);
}

/// We can't use instanceof here: hermes does not seem to generate the right
/// prototype chain, so we'll check the error message instead.
export function assertThrows<T>(errorVariant: string, fn: () => T) {
  try {
    fn();
    fail("No error was thrown");
  } catch (e: any) {
    if (e instanceof Error) {
      // Good, success!
      assertEqual(
        getErrorName(e),
        errorVariant,
        "Error is thrown, but the wrong one",
      );
    } else {
      fail(`Something else was thrown: ${e}`);
    }
  }
}

function getErrorName(error: Error): string {
  const typeName = (error as any)._uniffiTypeName ?? "Error";
  const variantName = (error as any)._uniffiVariantName ?? "unknown";
  return `${typeName}.${variantName}`;
}
