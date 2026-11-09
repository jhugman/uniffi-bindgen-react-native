/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// To run:
//   cargo test -p uniffi-fixture-ext-types -- async_wasm

import { asyncTest } from "@/asserts";

import { getGuid, getNestedOuid, getOuid } from "@/generated/ext_types_custom";
import {
  CombinedType,
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
import {
  callTraitImpl,
  getConsumerRecord,
  getTraitImpl,
  ConsumerRecord,
  throwSharedError,
} from "@/generated/ext_types_consumer";
import {
  getProcMacroRecord,
  SharedEnum,
  SharedError,
  SharedObject,
  SharedProcMacroRecord,
  SharedTrait,
  SharedRecord,
} from "@/generated/ext_types_shared";

import "@/polyfills";
import { URL } from "@/converters";

(async () => {
  await asyncTest("CombinedType from ext_types round-trips", async (t) => {
    const ct: CombinedType = await getCombinedType(undefined);
    t.assertEqual(ct.sharedRecord.sval, "hello");
    t.assertEqual(ct.guid, "a-guid");
    t.assertEqual(ct.url, new URL("http://example.com/"));
    t.assertEqual(ct.remoteRecord.sval, "remote");

    const ct2 = await getCombinedType(ct);
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
    t.assertEqual(await getCombinedType(full), full);
    t.end();
  });

  await asyncTest(
    "SharedObject from ext_types_shared, made by ext_types",
    async (t) => {
      const o = await getSharedObject();
      t.assertEqual(await o.increment(), 1);
      t.assertEqual(await o.increment(), 2);
      t.end();
    },
  );

  await asyncTest(
    "SharedObject made in TypeScript, held by ext_types_consumer and ext_types",
    async (t) => {
      const obj = (await SharedObject.create()) as SharedObject;
      t.assertEqual(await obj.increment(), 1);

      const consumer = await getConsumerRecord(
        ConsumerRecord.create({
          maybeEnum: undefined,
          maybeTrait: undefined,
          maybeObject: obj,
        }),
      );
      t.assertNotNull(consumer.maybeObject);
      // Counts keep rising: the round trip hands back the same object.
      t.assertEqual(await consumer.maybeObject!.increment(), 2);

      const ob = await getObjectsType(
        ObjectsType.create({
          maybeTrait: undefined,
          maybeObject: obj,
          consumer,
        }),
      );
      t.assertNotNull(ob.maybeObject);
      t.assertEqual(await ob.maybeObject!.increment(), 3);
      t.assertEqual(await obj.increment(), 4);
      t.end();
    },
  );

  await asyncTest(
    "SharedTrait from ext_types_shared, ObjectsType from ext_types, ConsumerRecord from ext_types_consumer passing a consumer object",
    async (t) => {
      const ti = await getTraitImpl();
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

      t.assertNull((await getObjectsType(undefined)).maybeObject);
      t.assertNotNull((await getObjectsType(ob)).maybeTrait);
      t.assertNull(await getSharedTrait(undefined));
      t.end();
    },
  );

  await asyncTest(
    "Url from ext_types_custom round-trips through ext_types",
    async (t) => {
      const url = new URL("http://example.com/");
      t.assertEqual(await getUrl(url), url);
      t.assertEqual(await getMaybeUrl(url), url);
      t.assertNull(await getMaybeUrl(undefined));
      t.assertEqual(await getUrls([url]), [url]);
      t.assertEqual(await getMaybeUrls([url, undefined]), [url, undefined]);
      t.end();
    },
  );

  await asyncTest(
    "Guid and Ouid from ext_types_custom, and the custom types ext_types builds on them",
    async (t) => {
      t.assertEqual(await getGuid("guid"), "guid");
      t.assertEqual(await getOuid("ouid"), "ouid");
      t.assertEqual(await getImportedOuid("ouid"), "ouid");
      t.assertEqual(await getNestedOuid("ouid"), "ouid");
      t.assertEqual(await getImportedNestedGuid(undefined), "nested");
      t.assertEqual(
        await getNestedExternalOuid(undefined),
        "nested-external-ouid",
      );
      t.end();
    },
  );

  await asyncTest(
    "SharedRecord from ext_types_shared round-trips through ext_types",
    async (t) => {
      t.assertEqual(
        (await getSharedRecord(SharedRecord.create({ sval: "hello" }))).sval,
        "hello",
      );
      t.assertEqual(
        (await getMaybeSharedRecord(SharedRecord.create({ sval: "hello" })))
          ?.sval,
        "hello",
      );
      t.assertNull(await getMaybeSharedRecord(undefined));
      t.assertEqual(
        await getSharedRecords([SharedRecord.create({ sval: "hello" })]),
        [SharedRecord.create({ sval: "hello" })],
      );
      t.assertEqual(
        (
          await getProcMacroRecord(
            SharedProcMacroRecord.create({ sval: "proc-macros" }),
          )
        ).sval,
        "proc-macros",
      );
      t.end();
    },
  );

  await asyncTest(
    "SharedEnum from ext_types_shared round-trips through ext_types",
    async (t) => {
      t.assertEqual(await getSharedEnum(SharedEnum.One), SharedEnum.One);
      t.assertEqual(await getMaybeSharedEnum(SharedEnum.One), SharedEnum.One);
      t.assertNull(await getMaybeSharedEnum(undefined));
      t.assertEqual(await getSharedEnums([SharedEnum.One, SharedEnum.Two]), [
        SharedEnum.One,
        SharedEnum.Two,
      ]);
      t.assertEqual(await getMaybeSharedEnums([SharedEnum.One, undefined]), [
        SharedEnum.One,
        undefined,
      ]);
      t.end();
    },
  );

  await asyncTest(
    "RemoteObject from ext_types_remote, made by ext_types",
    async (t) => {
      const remote = await getRemoteObject("foo");
      t.assertEqual(await remote.value(), "foo");
      t.end();
    },
  );

  await asyncTest(
    "SharedProcMacroRecord from ext_types_shared round-trips through ext_types and ext_types_shared",
    async (t) => {
      const t1 = SharedProcMacroRecord.create({ sval: "hello" });
      t.assertEqual(await getSharedProcMacroRecord(t1), t1);
      t.assertEqual(await getProcMacroRecord(t1), t1);
      t.end();
    },
  );

  await asyncTest(
    "SharedError from ext_types_shared is thrown by ext_types_consumer",
    async (t) => {
      await t.assertThrowsAsync(SharedError.instanceOf, async () => {
        await throwSharedError();
      });
      t.end();
    },
  );

  await asyncTest(
    "SharedTrait from ext_types_shared, from getTraitImpl in ext_types_consumer",
    async (t) => {
      const ti: SharedTrait = await getTraitImpl();
      t.assertEqual(await ti.hello(), "consumer trait impl says hello");
      const consumer = ConsumerRecord.create({
        maybeEnum: undefined,
        maybeTrait: ti,
        maybeObject: undefined,
      });
      const fromConsumer = (await getConsumerRecord(consumer)).maybeTrait;
      t.assertNotNull(fromConsumer);
      t.assertEqual(
        await fromConsumer!.hello(),
        "consumer trait impl says hello",
      );

      const ob = ObjectsType.create({
        maybeTrait: ti,
        maybeObject: undefined,
        consumer,
      });
      const fromObjects = (await getObjectsType(ob)).maybeTrait;
      t.assertNotNull(fromObjects);
      t.assertEqual(
        await fromObjects!.hello(),
        "consumer trait impl says hello",
      );

      const viaShared = await getSharedTrait(ti);
      t.assertNotNull(viaShared);
      t.assertEqual(await viaShared!.hello(), "consumer trait impl says hello");
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
