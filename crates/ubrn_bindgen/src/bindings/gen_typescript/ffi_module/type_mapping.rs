/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use uniffi_bindgen::pipeline::general;

pub(super) use crate::bindings::gen_typescript::type_mapping::ffi_type_to_ts;

/// Overrides for the native module interface (C++/JSI boundary).
pub(super) fn ffi_type_to_ts_native(ffi_type: &general::FfiType) -> String {
    match ffi_type {
        general::FfiType::Handle(_) => "UniffiGcObject".into(),
        general::FfiType::ForeignBytes => "Uint8Array".into(),
        general::FfiType::RustBuffer(_) => "string".into(),
        _ => ffi_type_to_ts(ffi_type),
    }
}
