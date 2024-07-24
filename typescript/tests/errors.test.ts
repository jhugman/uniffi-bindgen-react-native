/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { Asserts, test, xtest } from "../testing/asserts";
import { console } from "../testing/hermes";

// This test is showing and experimenting with the limitations of hermes.
// The actual Uniffi error may not stay as this implementation.
class UniffiError extends Error {
  constructor(
    private __typename: string,
    private __variant: string,
    message?: string,
  ) {
    super(message);
  }

  static instanceOf(err: any): err is UniffiError {
    return err instanceof Error && (err as any).__typename !== undefined;
  }
}

const MyError = (() => {
  class ThisException extends UniffiError {
    constructor() {
      super("ComplexError", "ThisException");
    }
    static instanceOf(e: any): e is ThisException {
      return instanceOf(e) && (e as any).__variant === "ThisException";
    }
  }
  class OtherException extends UniffiError {
    constructor() {
      super("ComplexError", "OtherException");
    }
    static instanceOf(e: any): e is OtherException {
      return instanceOf(e) && (e as any).__variant === "OtherException";
    }
  }
  function instanceOf(e: any): e is ThisException | OtherException {
    return (e as any).__typename === "ComplexError";
  }

  return {
    ThisException,
    OtherException,
    instanceOf,
  };
})();

test("Typesecript type generation", (t) => {
  // Create a type that represents the instance types of the properties of MyError, excluding 'instanceOf'
  type MyErrorType = InstanceType<
    (typeof MyError)[keyof Omit<typeof MyError, "instanceOf">]
  >;
  const err: MyErrorType = new MyError.ThisException();
});

test("Vanilla instanceof tests", (t) => {
  const err = new MyError.ThisException();
  t.assertTrue(err instanceof Error, `err is Error`);

  // If this every fails, then hermes now supports instanceof checks
  // for subclasses. This opens up the possiblility of simplifying the
  // generated error classes, and error handling logic.
  //
  // At that point, we need to: raise a github issue to track that work,
  // and then flip these tests to assertTrue.
  t.assertFalse(err instanceof UniffiError, `err is UniffiError`);
  t.assertFalse(err instanceof MyError.ThisException, `err is ThisException`);
});

test("Vanilla instanceof tests with constructors", (t) => {
  const err = new MyError.ThisException();
  t.assertTrue(err instanceof Error, `err is Error`);

  // If this every fails, then hermes now supports instanceof checks
  // for subclasses. This opens up the possiblility of simplifying the
  // generated error classes, and error handling logic.
  //
  // At that point, we need to: raise a github issue to track that work,
  // and then flip these tests to assertEqual.
  t.assertNotEqual(
    err.constructor,
    MyError.ThisException,
    `err is ThisException`,
  );
});

test("Dynamic instanceof tests", (t) => {
  const err = new MyError.ThisException();
  const myType = MyError.ThisException;
  t.assertTrue(MyError.instanceOf(err), `err is MyError`);
  t.assertTrue(MyError.ThisException.instanceOf(err), `err is ThisException`);

  const myInstanceOf = myType.instanceOf;
  t.assertTrue(myInstanceOf(err), `Dynamic instanceOf`);
});

test("Higher order instanceof tests", (t) => {
  const err = new MyError.ThisException();
  const myType = MyError.ThisException;

  function checkInstanceOf<T>(e: any, instanceOf: (e: any) => boolean): e is T {
    return instanceOf(e);
  }

  const myInstanceOf = myType.instanceOf;
  t.assertTrue(checkInstanceOf(err, myInstanceOf), `checkInstanceOf`);
});

test("Higher order type tests", (t) => {
  const err = new MyError.ThisException();
  const myType = MyError.ThisException;

  function checkType<T>(e: any, type: new () => T): e is T {
    return e instanceof type;
  }

  // If this every fails, then hermes now supports instanceof checks
  // for subclasses. This opens up the possiblility of simplifying the
  // generated error classes, and error handling logic.
  //
  // At that point, we need to: raise a github issue to track that work,
  // and then flip the test to assertTrue.
  t.assertFalse(checkType(err, myType), `checkType`);
});
