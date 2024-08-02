/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "@/asserts";
import {
  ErrorInterface,
  ErrorTrait,
  Exception,
  FlatInner,
  getError,
  Inner,
  oops,
  oopsEnum,
  oopsNowrap,
  oopsTuple,
  RichError,
  throwRich,
  toops,
  TupleError,
} from "../../generated/error_types";
import { UniffiThrownObject } from "../../../../typescript/src/errors";

test("oops: an error _object_ is thrown", (t) => {
  t.assertThrows(
    (error) => {
      t.assertTrue(
        UniffiThrownObject.instanceOf(error),
        "error is not wrapped in UniffiThrownObject",
      );
      if (ErrorInterface.hasInner(error)) {
        const instance = ErrorInterface.getInner(error);
        t.assertEqual(
          instance.chain(),
          ["because uniffi told me so,oops"],
          "chain is not equal",
        );
      } else {
        t.fail("No inner!");
      }
      // t.assertEqual(error.toString(), `ErrorInterface { e: ${msg} }`);
      // assert(String(describing: e) == msg)
      // assert(error.localizedDescription == "ErrorInterface { e: \(msg) }")
      return ErrorInterface.hasInner(error);
    },
    () => oops(),
  );
});

test("oopsEnum 0", (t) => {
  t.assertThrows(
    (error) => {
      // assert(String(describing: error) == "Oops")
      // assert(String(reflecting: error) == "error_types.Error.Oops")
      // assert(error.localizedDescription == "error_types.Error.Oops")
      return Exception.Oops.instanceOf(error);
    },
    () => oopsEnum(0),
  );
});

test("oopsEnum 1", (t) => {
  t.assertThrows(
    (error) => {
      if (Exception.Value.instanceOf(error)) {
        t.assertEqual(error.toString(), "Error: Exception.Value");
        const inner = Exception.Value.getInner(error);
        t.assertEqual(inner.value, "value");
        // assert(String(describing: error) == "Value(value: \"value\")")
        // assert(String(reflecting: error) == "error_types.Error.Value(value: \"value\")")
        // assert(error.localizedDescription == "error_types.Error.Value(value: \"value\")")
        return true;
      }
      return false;
    },
    () => oopsEnum(1),
  );
});

test("oopsEnum 2", (t) => {
  t.assertThrows(
    (error) => {
      if (Exception.IntValue.instanceOf(error)) {
        t.assertEqual(error.data.value, 2);
        // assert(String(describing: error) == "IntValue(value: 2)")
        // assert(String(reflecting: error) == "error_types.Error.IntValue(value: 2)")
        // assert(error.localizedDescription == "error_types.Error.IntValue(value: 2)")
        return true;
      }
      return false;
    },
    () => oopsEnum(2),
  );
});

test("oopsEnum 3", (t) => {
  t.assertThrows(
    (error) => {
      if (Exception.FlatInnerError.instanceOf(error)) {
        t.assertEqual(error.toString(), "Error: Exception.FlatInnerError");
        t.assertTrue(FlatInner.CaseA.instanceOf(error.data.error));
        t.assertEqual(
          error.data.error.toString(),
          "Error: FlatInner.CaseA: inner",
        );
        // assert(String(describing: e) == "FlatInnerError(error: error_types.FlatInner.CaseA(message: \"inner\"))")
        // assert(String(reflecting: e) == "error_types.Error.FlatInnerError(error: error_types.FlatInner.CaseA(message: \"inner\"))")
        return true;
      }
      return false;
    },
    () => oopsEnum(3),
  );
});

test("oopsEnum 4", (t) => {
  t.assertThrows(
    (error) => {
      if (Exception.FlatInnerError.instanceOf(error)) {
        t.assertEqual(error.toString(), "Error: Exception.FlatInnerError");
        if (Exception.FlatInnerError.hasInner(error)) {
          const inner = Exception.FlatInnerError.getInner(error);
          if (FlatInner.CaseB.instanceOf(inner)) {
            t.assertEqual(inner.error.toString(), "NonUniffiTypeValue: value");
            return true;
          }
        }
        // assert(String(describing: e) == "FlatInnerError(error: error_types.FlatInner.CaseB(message: \"NonUniffiTypeValue: value\"))")
        // assert(String(reflecting: e) == "error_types.Error.FlatInnerError(error: error_types.FlatInner.CaseB(message: \"NonUniffiTypeValue: value\"))")
        return true;
      }
      return false;
    },
    () => oopsEnum(4),
  );
});

test("oopsEnum 5", (t) => {
  t.assertThrows(
    (error) => {
      t.assertEqual("Error: Exception.InnerError", error.toString());
      if (Exception.InnerError.instanceOf(error)) {
        // assert(String(describing: e) == "InnerError(error: error_types.Inner.CaseA(\"inner\"))")
        if (Exception.InnerError.hasInner(error)) {
          const inner = Exception.InnerError.getInner(error);
          t.assertTrue(Inner.CaseA.instanceOf(inner.error));
          t.assertEqual(["inner"], Inner.CaseA.getInner(inner.error));
          return true;
        }
      }
      return false;
    },
    () => oopsEnum(5),
  );
});

test("oopsTuple 0 - throws enum variants containing tuples", (t) => {
  t.assertThrows(
    (error) => {
      t.assertEqual("Error: TupleError.Oops", error.toString());
      if (TupleError.Oops.instanceOf(error)) {
        t.assertEqual("oops", error.data[0]);
        return true;
      }
      return false;
    },
    () => oopsTuple(0),
  );
});

test("oopsTuple 1  - throws enum variants containing tuples", (t) => {
  t.assertThrows(
    (error) => {
      t.assertEqual("Error: TupleError.Value", error.toString());
      if (TupleError.Value.instanceOf(error)) {
        t.assertEqual(1, error.data[0]);
        return true;
      }
      return false;
    },
    () => oopsTuple(1),
  );
});

test("oopsNowrap - throws an object not wrapped in an Arc", (t) => {
  t.assertThrows(
    (error) => {
      return (
        ErrorInterface.hasInner(error) &&
        ErrorInterface.getInner(error) !== undefined
      );
    },
    () => oopsNowrap(),
  );
});

test("toops – throws an error which is a trait implementation", (t) => {
  t.assertThrows(
    (error) => {
      t.assertEqual("Error: ErrorTrait", error.toString());
      if (ErrorTrait.hasInner(error)) {
        const inner = ErrorTrait.getInner(error);
        t.assertEqual(inner.msg(), "trait-oops");
        return true;
      }
      return false;
    },
    () => toops(),
  );
});

test("throwRich – throws an object wrapped in a UniffiThrownObject<T>", (t) => {
  const message = "Rich message";
  t.assertThrows(
    (error) => {
      t.assertEqual(
        `Error: RichError: RichError { e: \"${message}\" }`,
        error.toString(),
      );
      if (RichError.hasInner(error)) {
        const inner = RichError.getInner(error);
        t.assertEqual(`RichError: "${message}"`, inner.toString());
        return true;
      }
      return false;
    },
    () => throwRich(message),
  );
});

test("getError", (t) => {
  const e = getError("the error");
  t.assertEqual("the error", e.toString());
  t.assertEqual(["the error"], e.chain());
  // This not thrown as an error, so isn't wrapper in a UniffiThrownObject<T> class…
  t.assertFalse(ErrorInterface.hasInner(e));
  // … or an error…
  t.assertFalse(e instanceof Error);
  // … but it is an ErrorInterface.
  t.assertTrue(ErrorInterface.instanceOf(e));
});
