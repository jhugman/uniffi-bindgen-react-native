/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export type FfiTypeDesc =
  | { tag: "UInt8" }
  | { tag: "Int8" }
  | { tag: "UInt16" }
  | { tag: "Int16" }
  | { tag: "UInt32" }
  | { tag: "Int32" }
  | { tag: "UInt64" }
  | { tag: "Int64" }
  | { tag: "Float32" }
  | { tag: "Float64" }
  | { tag: "Handle" }
  | { tag: "RustBuffer" }
  | { tag: "ForeignBytes" }
  | { tag: "RustCallStatus" }
  | { tag: "VoidPointer" }
  | { tag: "Void" }
  | { tag: "Callback"; name: string }
  | { tag: "Struct"; name: string }
  | { tag: "Reference"; inner: FfiTypeDesc }
  | { tag: "MutReference"; inner: FfiTypeDesc };

// Tag factories for the `DEFINITIONS` table. Players read `.tag`, plus `.name`
// for Callback/Struct and `.inner` for Reference/MutReference; `.inner`
// mirrors the napi parser (`runtimes/napi/src/register/spec_from_js.rs`).
export const FfiType = {
  UInt8: { tag: "UInt8" } as const,
  Int8: { tag: "Int8" } as const,
  UInt16: { tag: "UInt16" } as const,
  Int16: { tag: "Int16" } as const,
  UInt32: { tag: "UInt32" } as const,
  Int32: { tag: "Int32" } as const,
  UInt64: { tag: "UInt64" } as const,
  Int64: { tag: "Int64" } as const,
  Float32: { tag: "Float32" } as const,
  Float64: { tag: "Float64" } as const,
  Handle: { tag: "Handle" } as const,
  RustBuffer: { tag: "RustBuffer" } as const,
  ForeignBytes: { tag: "ForeignBytes" } as const,
  RustCallStatus: { tag: "RustCallStatus" } as const,
  VoidPointer: { tag: "VoidPointer" } as const,
  Void: { tag: "Void" } as const,
  Callback: (name: string) => ({ tag: "Callback", name }) as const,
  Struct: (name: string) => ({ tag: "Struct", name }) as const,
  Reference: (inner: FfiTypeDesc) => ({ tag: "Reference", inner }) as const,
  MutReference: (inner: FfiTypeDesc) =>
    ({ tag: "MutReference", inner }) as const,
};

/** One field of a `#[repr(C)]` struct, in declaration order. */
export interface FieldDesc {
  name: string;
  type: FfiTypeDesc;
}

/** Present on a future-starting export when the player runs the poll loop
 * itself. The registered entry then takes `(...args, status, signal)` and
 * returns a Promise of the completed value, whose FFI type is `ret`. */
export interface AsyncFunctionDef {
  poll: string;
  cancel: string;
  complete: string;
  free: string;
  ret: FfiTypeDesc;
}

/** One FFI export. `hasRustCallStatus` adds a trailing status pointer. */
export interface FunctionDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
  async?: AsyncFunctionDef;
}

/** One function Rust calls into JavaScript. `outReturn` means the C
 * signature returns through a trailing out-pointer rather than by value. */
export interface CallbackDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
  outReturn?: boolean;
}

/** The table the bindgen emits as `DEFINITIONS`; every player consumes it. */
export interface ModuleDefinitions {
  symbols: {
    rustbuffer_alloc: string;
    rustbuffer_free: string;
    rustbuffer_from_bytes: string;
  };
  functions: Record<string, FunctionDef>;
  callbacks: Record<string, CallbackDef>;
  structs: Record<string, FieldDesc[]>;
}

type MergedKey = "functions" | "callbacks" | "structs";

/** One table for a library of several namespaces, so one player and one port
 * serve them all. Function names are crate-prefixed, and each foreign trait's
 * callbacks and structs carry its namespace, so only uniffi's generic shapes
 * share a name across namespaces. A shared name whose definitions differ
 * throws; that takes two names that concatenate alike, such as namespace `app`
 * with trait `CoreListener` and namespace `app_core` with trait `Listener`.
 * Definitions are compared as JSON, so as the bindgen writes them. */
export function uniffiMergeDefinitions(
  tables: readonly ModuleDefinitions[],
): ModuleDefinitions {
  if (tables.length === 0) {
    throw new Error("uniffiMergeDefinitions: no tables to merge");
  }
  const merge = <K extends MergedKey>(key: K): ModuleDefinitions[K] => {
    // No prototype, so `constructor` and friends are names like any other.
    const out: Record<string, unknown> = Object.create(null);
    for (const table of tables) {
      for (const [name, def] of Object.entries(table[key])) {
        if (name in out && JSON.stringify(out[name]) !== JSON.stringify(def)) {
          throw new Error(
            `uniffiMergeDefinitions: ${key} ${name} differs between namespaces`,
          );
        }
        out[name] = def;
      }
    }
    return out as ModuleDefinitions[K];
  };
  return {
    // One library has one allocator: any namespace's buffer functions serve all.
    symbols: tables[0].symbols,
    functions: merge("functions"),
    callbacks: merge("callbacks"),
    structs: merge("structs"),
  };
}
