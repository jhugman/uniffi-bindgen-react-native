/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import type { FfiTypeDesc, FieldDesc, ModuleDefinitions } from "@ubjs/core";

export type CallbackLifetime = "persistent" | "invocation";

export type ValuePlan =
  | { kind: "pass" }
  | { kind: "handle" }
  | { kind: "buffer" }
  | { kind: "callback"; name: string; lifetime: CallbackLifetime }
  | {
      kind: "struct";
      name: string;
      fields: { name: string; plan: ValuePlan }[];
    };

export interface FunctionPlan {
  name: string;
  args: ValuePlan[];
  ret: ValuePlan;
  hasRustCallStatus: boolean;
}

export interface CallbackPlan {
  name: string;
  args: ValuePlan[];
  ret: ValuePlan;
  retTag: FfiTypeDesc["tag"];
  hasRustCallStatus: boolean;
  outReturn: boolean;
}

export interface ModulePlan {
  functions: Map<string, FunctionPlan>;
  callbacks: Map<string, CallbackPlan>;
}

const SCALAR_TAGS = new Set([
  "UInt8",
  "Int8",
  "UInt16",
  "Int16",
  "UInt32",
  "Int32",
  "UInt64",
  "Int64",
  "Float32",
  "Float64",
  "Void",
]);

interface Site {
  defs: ModuleDefinitions;
  where: string; // "functions.f arg 0", for error messages
  lifetime: CallbackLifetime;
  asField: boolean;
  outReturn: boolean;
}

function fail(site: Site, what: string): never {
  throw new Error(`message-channel: ${site.where}: ${what}`);
}

function planValue(t: FfiTypeDesc, site: Site): ValuePlan {
  const tag = (t as { tag?: unknown }).tag;
  if (typeof tag !== "string") fail(site, `unknown tag ${JSON.stringify(tag)}`);
  if (SCALAR_TAGS.has(tag)) return { kind: "pass" };
  switch (t.tag) {
    case "Handle":
      return { kind: "handle" };
    case "RustBuffer":
      return { kind: "buffer" };
    case "RustCallStatus":
      // Only ever a field of ForeignFutureResult*; as an argument it is
      // expressed by hasRustCallStatus instead.
      if (!site.asField)
        fail(site, "RustCallStatus is not an argument; use hasRustCallStatus");
      return { kind: "pass" };
    case "Callback":
      if (!(t.name in site.defs.callbacks))
        fail(site, `callback "${t.name}" is not in the table`);
      return { kind: "callback", name: t.name, lifetime: site.lifetime };
    case "Struct":
      return planStruct(t.name, site);
    case "Reference":
    case "MutReference":
      if (t.inner.tag !== "Struct")
        fail(
          site,
          `${t.tag} to ${(t.inner as { tag: string }).tag} is not supported`,
        );
      return planStruct(t.inner.name, site);
    case "VoidPointer":
      // uniffi declares a ()-returning vtable method's C out-return pointer as *mut c_void; nothing crosses.
      if (site.outReturn) return { kind: "pass" };
      fail(site, `${t.tag} is not supported over a channel`);
    case "ForeignBytes":
      fail(site, `${t.tag} is not supported over a channel`);
    default:
      fail(site, `unknown tag "${tag}"`);
  }
}

function planStruct(name: string, site: Site): ValuePlan {
  const fields: FieldDesc[] | undefined = site.defs.structs[name];
  if (!fields) fail(site, `struct "${name}" is not in the table`);
  return {
    kind: "struct",
    name,
    fields: fields.map((f) => ({
      name: f.name,
      plan: planValue(f.type, {
        ...site,
        where: `${site.where} field ${f.name}`,
        asField: true,
        outReturn: false,
      }),
    })),
  };
}

export function compilePlan(defs: ModuleDefinitions): ModulePlan {
  const functions = new Map<string, FunctionPlan>();
  for (const [name, def] of Object.entries(defs.functions)) {
    const site = (where: string): Site => ({
      defs,
      where,
      lifetime: "persistent",
      asField: false,
      outReturn: false,
    });
    functions.set(name, {
      name,
      args: def.args.map((t, i) =>
        planValue(t, site(`functions.${name} arg ${i}`)),
      ),
      ret: planValue(def.ret, site(`functions.${name} ret`)),
      hasRustCallStatus: def.hasRustCallStatus,
    });
  }
  const callbacks = new Map<string, CallbackPlan>();
  for (const [name, def] of Object.entries(defs.callbacks)) {
    const site = (where: string): Site => ({
      defs,
      where,
      lifetime: "invocation",
      asField: false,
      outReturn: false,
    });
    callbacks.set(name, {
      name,
      args: def.args.map((t, i) =>
        planValue(t, site(`callbacks.${name} arg ${i}`)),
      ),
      ret: planValue(def.ret, {
        ...site(`callbacks.${name} ret`),
        outReturn: def.outReturn === true,
      }),
      retTag: def.ret.tag,
      hasRustCallStatus: def.hasRustCallStatus,
      outReturn: def.outReturn === true,
    });
  }
  return { functions, callbacks };
}
