/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use uniffi_bindgen::pipeline::general;

/// Map a `general::FfiType` to the player's `FfiType.XXX` expression string.
///
/// These correspond to the tags defined in `runtimes/napi/lib.js`.
pub(super) fn ffi_type_to_player(ffi_type: &general::FfiType) -> String {
    match ffi_type {
        general::FfiType::UInt8 => "FfiType.UInt8".into(),
        general::FfiType::Int8 => "FfiType.Int8".into(),
        general::FfiType::UInt16 => "FfiType.UInt16".into(),
        general::FfiType::Int16 => "FfiType.Int16".into(),
        general::FfiType::UInt32 => "FfiType.UInt32".into(),
        general::FfiType::Int32 => "FfiType.Int32".into(),
        general::FfiType::UInt64 => "FfiType.UInt64".into(),
        general::FfiType::Int64 => "FfiType.Int64".into(),
        general::FfiType::Float32 => "FfiType.Float32".into(),
        general::FfiType::Float64 => "FfiType.Float64".into(),
        general::FfiType::Handle(_) => "FfiType.Handle".into(),
        general::FfiType::RustBuffer(_) => "FfiType.RustBuffer".into(),
        general::FfiType::RustCallStatus => "FfiType.RustCallStatus".into(),
        general::FfiType::ForeignBytes => "FfiType.ForeignBytes".into(),
        general::FfiType::VoidPointer => "FfiType.VoidPointer".into(),
        general::FfiType::Function(name) => {
            format!("FfiType.Callback(\"{}\")", name.0)
        }
        general::FfiType::Struct(name) => {
            format!("FfiType.Struct(\"{}\")", name.0)
        }
        general::FfiType::Reference(inner) => {
            format!("FfiType.Reference({})", ffi_type_to_player(inner))
        }
        general::FfiType::MutReference(inner) => {
            format!("FfiType.MutReference({})", ffi_type_to_player(inner))
        }
    }
}
