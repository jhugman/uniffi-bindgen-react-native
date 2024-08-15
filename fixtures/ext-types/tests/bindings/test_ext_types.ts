/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { test } from "@/asserts";

import module1 from "../../generated/custom_types";
import module2, {
  getGuid,
  getNestedOuid,
  getOuid,
} from "../../generated/ext_types_custom";
import module3, {
  CombinedType,
  ExternalCrateInterfaceInterface,
  getCombinedType,
  getExternalCrateInterface,
  getImportedNestedGuid,
  getImportedOuid,
  getMaybeUniffiOneEnum,
  getMaybeUniffiOneEnums,
  getMaybeUniffiOneType,
  getMaybeUrl,
  getMaybeUrls,
  getNestedExternalOuid,
  getObjectsType,
  getUniffiOneEnum,
  getUniffiOneEnums,
  getUniffiOneProcMacroType,
  getUniffiOneTrait,
  getUniffiOneType,
  getUniffiOneTypes,
  getUrl,
  getUrls,
  ObjectsType,
} from "../../generated/imported_types_lib";
import module4, {
  getSubType,
  getTraitImpl,
  SubLibType,
} from "../../generated/imported_types_sublib";
import module5, {
  getMyProcMacroType,
  UniffiOneEnum,
  UniffiOneProcMacroType,
  UniffiOneTrait,
  UniffiOneType,
} from "../../generated/uniffi_one_ns";

import { URL, console } from "@/hermes";

module1.initialize();
module2.initialize();
module3.initialize();
module4.initialize();
module5.initialize();

// import imported_types_lib
// import Foundation

test("combinedType from lib", (t) => {
  const ct: CombinedType = getCombinedType(undefined);
  t.assertEqual(ct.uot.sval, "hello");
  t.assertEqual(ct.guid, "a-guid");
  t.assertEqual(ct.url, new URL("http://example.com/"));
  t.assertEqual(ct.ecd.sval, "ecd");

  const ct2 = getCombinedType(ct);
  t.assertEqual(ct, ct2);
});

test("Getting a UniffiOneTrait from uniffi-one-ns, from getTraitImpl function in sublib", (t) => {
  const ti: UniffiOneTrait = getTraitImpl();
  t.assertEqual(ti.hello(), "sub-lib trait impl says hello");
  const sub = SubLibType.create({
    maybeEnum: undefined,
    maybeTrait: ti,
    maybeInterface: undefined,
  });
  t.assertNotNull(getSubType(sub).maybeTrait);
});

test("UniffiOneTrait from uniffi-one-ns, ObjectsType record from lib, SubLibType record from sublib passing sublib object", (t) => {
  const ti = getTraitImpl();
  const sub = SubLibType.create({
    maybeEnum: undefined,
    maybeTrait: ti,
    maybeInterface: undefined,
  });

  const ob = ObjectsType.create({
    maybeTrait: ti,
    maybeInterface: undefined,
    sub,
  });

  t.assertNull(getObjectsType(undefined).maybeInterface);
  t.assertNotNull(getObjectsType(ob).maybeTrait);
  t.assertNull(getUniffiOneTrait(undefined));
});

test("getUrl from lib using custom-type from custom-types-example", (t) => {
  const url = new URL("http://example.com/");
  t.assertEqual(getUrl(url), url);
  t.assertEqual(getMaybeUrl(url), url);
  t.assertNull(getMaybeUrl(undefined));
  t.assertEqual(getUrls([url]), [url]);
  t.assertEqual(getMaybeUrls([url, undefined]), [url, undefined]);
});

test("Calling in to custom-types-example and lib", (t) => {
  t.assertEqual(getGuid("guid"), "guid");
  t.assertEqual(getOuid("ouid"), "ouid");
  t.assertEqual(getImportedOuid("ouid"), "ouid");
  t.assertEqual(getNestedOuid("ouid"), "ouid");
  t.assertEqual(getImportedNestedGuid(undefined), "nested");
  t.assertEqual(getNestedExternalOuid(undefined), "nested-external-ouid");
});

test("UniffiOneType record from uniffi-one, roundtrip function from lib", (t) => {
  t.assertEqual(
    getUniffiOneType(UniffiOneType.create({ sval: "hello" })).sval,
    "hello",
  );
  t.assertEqual(
    getMaybeUniffiOneType(UniffiOneType.create({ sval: "hello" }))?.sval,
    "hello",
  );
  t.assertNull(getMaybeUniffiOneType(undefined));
  t.assertEqual(getUniffiOneTypes([UniffiOneType.create({ sval: "hello" })]), [
    UniffiOneType.create({ sval: "hello" }),
  ]);
  t.assertEqual(
    getMyProcMacroType(UniffiOneProcMacroType.create({ sval: "proc-macros" }))
      .sval,
    "proc-macros",
  );
});

test("UniffiOneEnum enum from uniffi-one, roundtrip function from lib", (t) => {
  t.assertEqual(getUniffiOneEnum(UniffiOneEnum.One), UniffiOneEnum.One);
  t.assertEqual(getMaybeUniffiOneEnum(UniffiOneEnum.One), UniffiOneEnum.One);
  t.assertNull(getMaybeUniffiOneEnum(undefined));
  t.assertEqual(getUniffiOneEnums([UniffiOneEnum.One, UniffiOneEnum.Two]), [
    UniffiOneEnum.One,
    UniffiOneEnum.Two,
  ]);
  t.assertEqual(getMaybeUniffiOneEnums([UniffiOneEnum.One, undefined]), [
    UniffiOneEnum.One,
    undefined,
  ]);
});

test("ExternalCrateInterface interface from uniffi-one-ns, roundtrip function from lib", (t) => {
  const foo: ExternalCrateInterfaceInterface = getExternalCrateInterface("foo");
  t.assertEqual(getExternalCrateInterface("foo").value(), "foo");
});

test("proc-macro fron uniffi-one-ns roundtripping via functions in lib", (t) => {
  const t1 = UniffiOneProcMacroType.create({ sval: "hello" });
  t.assertEqual(getUniffiOneProcMacroType(t1), t1);
  t.assertEqual(getMyProcMacroType(t1), t1);
});
