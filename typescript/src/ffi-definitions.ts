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

/** One FFI export. `hasRustCallStatus` adds a trailing status pointer. */
export interface FunctionDef {
  args: FfiTypeDesc[];
  ret: FfiTypeDesc;
  hasRustCallStatus: boolean;
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
