/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use heck::{ToLowerCamelCase, ToUpperCamelCase};
use uniffi_bindgen::interface::FfiType;

pub fn ffi_type_name_from_js(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(match ffi_type {
        FfiType::Reference(inner) => ffi_type_name_from_js(inner)?,
        _ => ffi_type_name(ffi_type)?,
    })
}

pub fn ffi_type_name_to_rust(ffi_type: &FfiType) -> Result<String, askama::Error> {
    ffi_type_name(ffi_type)
}

pub fn ffi_type_name(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(match ffi_type {
        FfiType::UInt8 => "uint8_t".into(),
        FfiType::Int8 => "int8_t".into(),
        FfiType::UInt16 => "uint16_t".into(),
        FfiType::Int16 => "int16_t".into(),
        FfiType::UInt32 => "uint32_t".into(),
        FfiType::Int32 => "int32_t".into(),
        FfiType::UInt64 => "uint64_t".into(),
        FfiType::Int64 => "int64_t".into(),
        FfiType::Float32 => "float".into(),
        FfiType::Float64 => "double".into(),
        FfiType::RustArcPtr(_) => "void *".into(),
        FfiType::RustBuffer(_) => "RustBuffer".into(),
        FfiType::ForeignBytes => "ForeignBytes".into(),
        FfiType::Callback(nm) => ffi_callback_name(nm)?,
        FfiType::Struct(nm) => ffi_struct_name(nm)?,
        FfiType::Handle => "/*handle*/ uint64_t".into(),
        FfiType::RustCallStatus => "RustCallStatus".into(),
        FfiType::Reference(inner) => format!("{} *", ffi_type_name(inner)?),
        FfiType::VoidPointer => "void *".into(), // ???
    })
}

pub fn var_name(nm: &str) -> Result<String, askama::Error> {
    Ok(nm.to_lower_camel_case())
}

pub fn ffi_callback_name(nm: &str) -> Result<String, askama::Error> {
    Ok(format!("Uniffi{}", nm.to_upper_camel_case()))
}

pub fn ffi_struct_name(nm: &str) -> Result<String, askama::Error> {
    Ok(format!("Uniffi{}", nm.to_upper_camel_case()))
}
