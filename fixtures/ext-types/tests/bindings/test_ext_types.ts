/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-ext-types -- jsi
//   cargo test -p uniffi-fixture-ext-types -- wasm

import { asyncTest, test } from "@/asserts";

import customNs, {
  getGuid,
  getNestedOuid,
  getOuid,
} from "@/generated/ext_types_custom";
import extTypes, {
  CombinedType,
  RemoteObjectInterface,
  getCombinedType,
  getRemoteObject,
  getImportedNestedGuid,
  getImportedOuid,
  getMaybeSharedEnum,
  getMaybeSharedEnums,
  getMaybeSharedRecord,
  getMaybeUrl,
  getMaybeUrls,
  getNestedExternalOuid,
  getObjectsType,
  getSharedEnum,
  getSharedEnums,
  getSharedObject,
  getSharedProcMacroRecord,
  getSharedTrait,
  getSharedRecord,
  getSharedRecords,
  getUrl,
  getUrls,
  ObjectsType,
} from "@/generated/ext_types";
import consumerNs, {
  callTraitImpl,
  getConsumerRecord,
  getTraitImpl,
  ConsumerRecord,
  throwSharedError,
} from "@/generated/ext_types_consumer";
import sharedNs, {
  getProcMacroRecord,
  SharedEnum,
  SharedError,
  SharedProcMacroRecord,
  SharedTrait,
  SharedRecord,
} from "@/generated/ext_types_shared";

import "@/polyfills";
import { URL } from "@/converters";

customNs.initialize();
extTypes.initialize();
consumerNs.initialize();
sharedNs.initialize();

test("CombinedType from ext_types round-trips", (t) => {
  const ct: CombinedType = getCombinedType(undefined);
  t.assertEqual(ct.sharedRecord.sval, "hello");
  t.assertEqual(ct.guid, "a-guid");
  t.assertEqual(ct.url, new URL("http://example.com/"));
  t.assertEqual(ct.remoteRecord.sval, "remote");

  const ct2 = getCombinedType(ct);
  t.assertEqual(ct, ct2);

  // Every Option set and every Vec non-empty, so the external converters
  // run inside Option and Vec.
  const full: CombinedType = {
    ...ct,
    maybeSharedRecord: SharedRecord.create({ sval: "maybe" }),
    urls: [new URL("http://example.com/a"), new URL("http://example.com/b")],
    maybeUrl: new URL("http://example.com/maybe"),
    maybeGuid: "maybe-guid",
  };
  t.assertEqual(getCombinedType(full), full);
});

test("SharedObject from ext_types_shared, made by ext_types", (t) => {
  const o = getSharedObject();
  t.assertEqual(o.increment(), 1);
  t.assertEqual(o.increment(), 2);
});

test("SharedTrait from ext_types_shared, ObjectsType from ext_types, ConsumerRecord from ext_types_consumer passing a consumer object", (t) => {
  const ti = getTraitImpl();
  const consumer = ConsumerRecord.create({
    maybeEnum: undefined,
    maybeTrait: ti,
    maybeObject: undefined,
  });

  const ob = ObjectsType.create({
    maybeTrait: ti,
    maybeObject: undefined,
    consumer,
  });

  t.assertNull(getObjectsType(undefined).maybeObject);
  t.assertNotNull(getObjectsType(ob).maybeTrait);
  t.assertNull(getSharedTrait(undefined));
});

test("Url from ext_types_custom round-trips through ext_types", (t) => {
  const url = new URL("http://example.com/");
  t.assertEqual(getUrl(url), url);
  t.assertEqual(getMaybeUrl(url), url);
  t.assertNull(getMaybeUrl(undefined));
  t.assertEqual(getUrls([url]), [url]);
  t.assertEqual(getMaybeUrls([url, undefined]), [url, undefined]);
});

test("Guid and Ouid from ext_types_custom, and the customs ext_types wraps", (t) => {
  t.assertEqual(getGuid("guid"), "guid");
  t.assertEqual(getOuid("ouid"), "ouid");
  t.assertEqual(getImportedOuid("ouid"), "ouid");
  t.assertEqual(getNestedOuid("ouid"), "ouid");
  t.assertEqual(getImportedNestedGuid(undefined), "nested");
  t.assertEqual(getNestedExternalOuid(undefined), "nested-external-ouid");
});

test("SharedRecord from ext_types_shared round-trips through ext_types", (t) => {
  t.assertEqual(
    getSharedRecord(SharedRecord.create({ sval: "hello" })).sval,
    "hello",
  );
  t.assertEqual(
    getMaybeSharedRecord(SharedRecord.create({ sval: "hello" }))?.sval,
    "hello",
  );
  t.assertNull(getMaybeSharedRecord(undefined));
  t.assertEqual(getSharedRecords([SharedRecord.create({ sval: "hello" })]), [
    SharedRecord.create({ sval: "hello" }),
  ]);
  t.assertEqual(
    getProcMacroRecord(SharedProcMacroRecord.create({ sval: "proc-macros" }))
      .sval,
    "proc-macros",
  );
});

test("SharedEnum from ext_types_shared round-trips through ext_types", (t) => {
  t.assertEqual(getSharedEnum(SharedEnum.One), SharedEnum.One);
  t.assertEqual(getMaybeSharedEnum(SharedEnum.One), SharedEnum.One);
  t.assertNull(getMaybeSharedEnum(undefined));
  t.assertEqual(getSharedEnums([SharedEnum.One, SharedEnum.Two]), [
    SharedEnum.One,
    SharedEnum.Two,
  ]);
  t.assertEqual(getMaybeSharedEnums([SharedEnum.One, undefined]), [
    SharedEnum.One,
    undefined,
  ]);
});

test("RemoteObject from ext_types_remote, made by ext_types", (t) => {
  const foo: RemoteObjectInterface = getRemoteObject("foo");
  t.assertEqual(getRemoteObject("foo").value(), "foo");
});

test("SharedProcMacroRecord from ext_types_shared round-trips through ext_types and ext_types_shared", (t) => {
  const t1 = SharedProcMacroRecord.create({ sval: "hello" });
  t.assertEqual(getSharedProcMacroRecord(t1), t1);
  t.assertEqual(getProcMacroRecord(t1), t1);
});

test("SharedError from ext_types_shared is thrown by ext_types_consumer", (t) => {
  t.assertThrows(SharedError.instanceOf, () => {
    throwSharedError();
  });
});

(async () => {
  await asyncTest(
    "SharedTrait from ext_types_shared, from getTraitImpl in ext_types_consumer",
    async (t) => {
      const ti: SharedTrait = getTraitImpl();
      t.assertEqual(await ti.hello(), "consumer trait impl says hello");
      const consumer = ConsumerRecord.create({
        maybeEnum: undefined,
        maybeTrait: ti,
        maybeObject: undefined,
      });
      t.assertNotNull(getConsumerRecord(consumer).maybeTrait);
      t.end();
    },
  );

  await asyncTest(
    "TypeScript implementing an external trait works",
    async (t) => {
      const tsImpl: SharedTrait = {
        async hello(): Promise<string> {
          return "hello from TypeScript implementing external trait";
        },
      };
      t.assertEqual(
        await callTraitImpl(tsImpl),
        "hello from TypeScript implementing external trait",
      );
      t.end();
    },
  );
})();
