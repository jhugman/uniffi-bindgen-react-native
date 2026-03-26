/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use heck::ToUpperCamelCase;
use uniffi_bindgen::pipeline::general;

/// Map an FFI type to its TypeScript representation.
pub(crate) fn ffi_type_to_ts(ffi_type: &general::FfiType) -> String {
    match ffi_type {
        general::FfiType::Int8 | general::FfiType::UInt8 => "number".into(),
        general::FfiType::Int16 | general::FfiType::UInt16 => "number".into(),
        general::FfiType::Int32 | general::FfiType::UInt32 => "number".into(),
        general::FfiType::Int64 | general::FfiType::UInt64 => "bigint".into(),
        general::FfiType::Float32 | general::FfiType::Float64 => "number".into(),
        general::FfiType::Handle(_) => "bigint".into(),
        general::FfiType::RustBuffer(_) => "Uint8Array".into(),
        general::FfiType::RustCallStatus => "UniffiRustCallStatus".into(),
        general::FfiType::ForeignBytes => "ForeignBytes".into(),
        general::FfiType::Function(name) => format!("Uniffi{}", name.0.to_upper_camel_case()),
        general::FfiType::Struct(name) => format!("Uniffi{}", name.0.to_upper_camel_case()),
        general::FfiType::Reference(inner) | general::FfiType::MutReference(inner) => {
            ffi_type_to_ts(inner)
        }
        general::FfiType::VoidPointer => "/*pointer*/ bigint".into(),
    }
}
