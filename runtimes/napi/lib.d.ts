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

export declare const FfiType: {
  UInt8: { readonly tag: "UInt8" };
  Int8: { readonly tag: "Int8" };
  UInt16: { readonly tag: "UInt16" };
  Int16: { readonly tag: "Int16" };
  UInt32: { readonly tag: "UInt32" };
  Int32: { readonly tag: "Int32" };
  UInt64: { readonly tag: "UInt64" };
  Int64: { readonly tag: "Int64" };
  Float32: { readonly tag: "Float32" };
  Float64: { readonly tag: "Float64" };
  Handle: { readonly tag: "Handle" };
  RustBuffer: { readonly tag: "RustBuffer" };
  ForeignBytes: { readonly tag: "ForeignBytes" };
  RustCallStatus: { readonly tag: "RustCallStatus" };
  VoidPointer: { readonly tag: "VoidPointer" };
  Void: { readonly tag: "Void" };
  Callback(name: string): { readonly tag: "Callback"; readonly name: string };
  Struct(name: string): { readonly tag: "Struct"; readonly name: string };
  Reference(inner: FfiTypeDesc): {
    readonly tag: "Reference";
    readonly inner: FfiTypeDesc;
  };
  MutReference(inner: FfiTypeDesc): {
    readonly tag: "MutReference";
    readonly inner: FfiTypeDesc;
  };
};

export { UniffiNativeModule } from "./index";
export {
  resolveLibPath,
  ResolveLibPathError,
  type ResolveLibPathOptions,
} from "./typescript/dist/resolve-lib";
