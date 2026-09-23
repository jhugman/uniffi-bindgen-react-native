/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Tag factories for the player `DEFINITIONS` object. The shim reads `.tag`
// (and `.name` for Callback/Struct, `.inner` for Reference/MutReference). This
// is the player subset of @ubjs/node's FfiType: the JSI shim handles scalars
// (Void..RustBuffer), Callback, Struct, and Reference/MutReference(Struct).
// `Reference`/`MutReference` wrap an inner FfiType (a pointer to it); generated
// vtable-init functions emit `Reference(Struct("...VTable..."))` for the
// vtable-pointer arg. The `.inner` key mirrors `@ubjs/node`'s canonical FfiType
// and the napi parser (`runtimes/napi/src/register/spec_from_js.rs`).
export const FfiType = {
  Void: { tag: "Void" } as const,
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
  Callback: (name: string) => ({ tag: "Callback", name }) as const,
  Struct: (name: string) => ({ tag: "Struct", name }) as const,
  Reference: (inner: unknown) => ({ tag: "Reference", inner }) as const,
  MutReference: (inner: unknown) => ({ tag: "MutReference", inner }) as const,
};
