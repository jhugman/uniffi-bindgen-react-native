/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-error-types -- async_wasm

import { asyncTest } from "@/asserts";
import {
  ErrorInterface,
  ErrorTrait,
  Exception,
  Exception_Tags,
  FlatInner,
  FlatInner_Tags,
  getError,
  Inner,
  oops,
  oopsCustom,
  oopsEnum,
  oopsNowrap,
  oopsTuple,
  RichError,
  throwRich,
  toops,
  TupleError,
} from "@/generated/error_types";
import { UniffiThrownObject } from "../../../../typescript/src/errors";

(async () => {
  await asyncTest("oops: an error _object_ is thrown", async (t) => {
    // The predicate is synchronous, so catch first, then await the bindings.
    let caught: unknown;
    try {
      await oops();
    } catch (e) {
      caught = e;
    }
    t.assertTrue(
      UniffiThrownObject.instanceOf(caught),
      "error is not wrapped in UniffiThrownObject",
    );
    if (ErrorInterface.hasInner(caught)) {
      const instance = ErrorInterface.getInner(caught);
      t.assertEqual(
        await instance.chain(),
        ["because uniffi told me so,oops"],
        "chain is not equal",
      );
    } else {
      t.fail("No inner!");
    }
    t.assertTrue(ErrorInterface.hasInner(caught));
    t.end();
  });

  await asyncTest("oopsEnum 0", async (t) => {
    await t.assertThrowsAsync(
      (error) => {
        // assert(String(describing: error) == "Oops")
        // assert(String(reflecting: error) == "error_types.Error.Oops")
        // assert(error.localizedDescription == "error_types.Error.Oops")
        return Exception.Oops.instanceOf(error);
      },
      () => oopsEnum(0),
    );
    t.end();
  });

  await asyncTest("oopsEnum 1", async (t) => {
    await t.assertThrowsAsync(
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
    t.end();
  });

  await asyncTest("oopsEnum 2", async (t) => {
    await t.assertThrowsAsync(
      (error) => {
        if (Exception.IntValue.instanceOf(error)) {
          t.assertEqual(error.inner.value, 2);
          // assert(String(describing: error) == "IntValue(value: 2)")
          // assert(String(reflecting: error) == "error_types.Error.IntValue(value: 2)")
          // assert(error.localizedDescription == "error_types.Error.IntValue(value: 2)")
          return true;
        }
        return false;
      },
      () => oopsEnum(2),
    );
    t.end();
  });

  await asyncTest("oopsEnum 3", async (t) => {
    await t.assertThrowsAsync(
      (error) => {
        if (Exception.instanceOf(error)) {
          switch (error.tag) {
            case Exception_Tags.FlatInnerError: {
              t.assertTrue(Exception.FlatInnerError.instanceOf(error));
              t.assertEqual(
                error.toString(),
                "Error: Exception.FlatInnerError",
              );
              const inner = error.inner.error;
              t.assertEqual(inner.toString(), "Error: FlatInner.CaseA: inner");
              switch (inner.tag) {
                case FlatInner_Tags.CaseA: {
                  t.assertTrue(FlatInner.CaseA.instanceOf(inner));
                  t.assertFalse(FlatInner.CaseB.instanceOf(inner));
                  return true;
                }
                default: {
                  // NOOP
                }
              }
            }
            default:
            // NOOP
          }
        }
        return false;
      },
      () => oopsEnum(3),
    );
    t.end();
  });

  await asyncTest("oopsEnum 4", async (t) => {
    await t.assertThrowsAsync(
      (error) => {
        if (Exception.FlatInnerError.instanceOf(error)) {
          t.assertEqual(error.toString(), "Error: Exception.FlatInnerError");
          if (Exception.FlatInnerError.hasInner(error)) {
            const innerError = Exception.FlatInnerError.getInner(error);
            const inner = innerError.error;
            if (FlatInner.instanceOf(inner)) {
              switch (inner.tag) {
                case FlatInner_Tags.CaseA:
                  t.assertTrue(FlatInner.CaseA.instanceOf(inner));
                  return true;
                case FlatInner_Tags.CaseB:
                  // We know that the Rust only produces this variant.
                  t.assertTrue(FlatInner.CaseB.instanceOf(inner));
                  return true;
              }
            }
          }
          // assert(String(describing: e) == "FlatInnerError(error: error_types.FlatInner.CaseB(message: \"NonUniffiTypeValue: value\"))")
          // assert(String(reflecting: e) == "error_types.Error.FlatInnerError(error: error_types.FlatInner.CaseB(message: \"NonUniffiTypeValue: value\"))")
        }
        return false;
      },
      () => oopsEnum(4),
    );
    t.end();
  });

  await asyncTest("oopsEnum 5", async (t) => {
    await t.assertThrowsAsync(
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
    t.end();
  });

  await asyncTest(
    "oopsTuple 0 - throws enum variants containing tuples",
    async (t) => {
      await t.assertThrowsAsync(
        (error) => {
          t.assertEqual("Error: TupleError.Oops", error.toString());
          if (TupleError.Oops.instanceOf(error)) {
            t.assertEqual("oops", error.inner[0]);
            return true;
          }
          return false;
        },
        () => oopsTuple(0),
      );
      t.end();
    },
  );

  await asyncTest(
    "oopsTuple 1  - throws enum variants containing tuples",
    async (t) => {
      await t.assertThrowsAsync(
        (error) => {
          t.assertEqual("Error: TupleError.Value", error.toString());
          if (TupleError.Value.instanceOf(error)) {
            t.assertEqual(1, error.inner[0]);
            return true;
          }
          return false;
        },
        () => oopsTuple(1),
      );
      t.end();
    },
  );

  await asyncTest(
    "oopsNowrap - throws an object not wrapped in an Arc",
    async (t) => {
      await t.assertThrowsAsync(
        (error) => {
          return (
            ErrorInterface.hasInner(error) &&
            ErrorInterface.getInner(error) !== undefined
          );
        },
        () => oopsNowrap(),
      );
      t.end();
    },
  );

  await asyncTest(
    "toops – throws an error which is a trait implementation",
    async (t) => {
      let caught: any;
      try {
        await toops();
      } catch (e) {
        caught = e;
      }
      t.assertEqual("Error: ErrorTrait", caught.toString());
      t.assertTrue(ErrorTrait.hasInner(caught));
      const inner = ErrorTrait.getInner(caught);
      t.assertEqual(await inner.msg(), "trait-oops");
      t.end();
    },
  );

  await asyncTest(
    "throwRich – throws an object wrapped in a UniffiThrownObject<T>",
    async (t) => {
      const message = "Rich message";
      let caught: unknown;
      try {
        await throwRich(message);
      } catch (e) {
        caught = e;
      }
      // The thrown object's message is not asserted: it is built from an async toDebugString(), so it reads "[object Promise]".
      if (RichError.hasInner(caught)) {
        const inner = RichError.getInner(caught);
        t.assertEqual(`RichError: "${message}"`, await inner.asyncToString());
      } else {
        t.fail(`expected RichError, got ${caught}`);
      }
      t.end();
    },
  );

  await asyncTest("getError", async (t) => {
    const e = (await getError("the error")) as ErrorInterface;
    t.assertEqual("the error", await e.asyncToString());
    t.assertEqual(["the error"], await e.chain());
    // This not thrown as an error, so isn't wrapper in a UniffiThrownObject<T> class…
    t.assertFalse(ErrorInterface.hasInner(e));
    // … or an error…
    t.assertFalse(e instanceof Error);
    // … but it is an ErrorInterface.
    t.assertTrue(ErrorInterface.instanceOf(e));
    t.end();
  });

  await asyncTest(
    "oops_custom - custom type wrapping enum used as error",
    async (t) => {
      await t.assertThrowsAsync(
        (error) => {
          if (TupleError.Oops.instanceOf(error)) {
            t.assertEqual(error.inner[0], "custom-oops");
            return true;
          }
          return false;
        },
        () => oopsCustom(0),
      );
      t.end();
    },
  );
})();
