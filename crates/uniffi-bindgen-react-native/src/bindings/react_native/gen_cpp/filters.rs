/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use uniffi_bindgen::interface::FfiType;

pub fn ffi_type_name_from_js(ffi_type: &FfiType) -> Result<String, askama::Error> {
    ffi_type_name(ffi_type)
}

pub fn ffi_type_name_to_rust(ffi_type: &FfiType) -> Result<String, askama::Error> {
    ffi_type_name(ffi_type)
}

fn ffi_type_name(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(match ffi_type {
        FfiType::UInt8 => "uint8_t",
        FfiType::Int8 => "int8_t",
        FfiType::UInt16 => "uint16_t",
        FfiType::Int16 => "int16_t",
        FfiType::UInt32 => "uint32_t",
        FfiType::Int32 => "int32_t",
        FfiType::UInt64 => "uint64_t",
        FfiType::Int64 => "int64_t",
        FfiType::Float32 => "float",
        FfiType::Float64 => "double",
        FfiType::RustArcPtr(_) => "void *",
        FfiType::RustBuffer(_) => "RustBuffer",
        FfiType::ForeignBytes => "ForeignBytes",
        FfiType::Callback(_) => "Callback",
        FfiType::Struct(_) => todo!(),
        FfiType::Handle => "Handle",
        FfiType::RustCallStatus => todo!(),
        FfiType::Reference(_) => todo!(),
        FfiType::VoidPointer => todo!(),
    }
    .to_owned())
}
