/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashSet;
use std::sync::LazyLock;

use heck::{ToLowerCamelCase, ToUpperCamelCase};
use uniffi_bindgen::{interface::FfiType, ComponentInterface};

use super::extensions::{CppComponentInterfaceExt as _, CppFfiTypeExt as _};

static CPP_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "alignas",
        "alignof",
        "and",
        "and_eq",
        "asm",
        "auto",
        "bitand",
        "bitor",
        "bool",
        "break",
        "case",
        "catch",
        "char",
        "char8_t",
        "char16_t",
        "char32_t",
        "class",
        "compl",
        "concept",
        "const",
        "consteval",
        "constexpr",
        "constinit",
        "const_cast",
        "continue",
        "co_await",
        "co_return",
        "co_yield",
        "decltype",
        "default",
        "delete",
        "do",
        "double",
        "dynamic_cast",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "false",
        "float",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "mutable",
        "namespace",
        "new",
        "noexcept",
        "not",
        "not_eq",
        "nullptr",
        "operator",
        "or",
        "or_eq",
        "private",
        "protected",
        "public",
        "register",
        "reinterpret_cast",
        "requires",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "static_assert",
        "static_cast",
        "struct",
        "switch",
        "template",
        "this",
        "thread_local",
        "throw",
        "true",
        "try",
        "typedef",
        "typeid",
        "typename",
        "union",
        "unsigned",
        "using",
        "virtual",
        "void",
        "volatile",
        "wchar_t",
        "while",
        "xor",
        "xor_eq",
    ])
});

pub fn ffi_type_name_from_js(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(match ffi_type {
        FfiType::MutReference(inner) | FfiType::Reference(inner) => ffi_type_name_from_js(inner)?,
        _ => ffi_type_name(ffi_type)?,
    })
}

pub fn cpp_namespace(ffi_type: &FfiType, ci: &ComponentInterface) -> Result<String, askama::Error> {
    Ok(ffi_type.cpp_namespace(ci))
}

pub fn bridging_namespace(
    ffi_type: &FfiType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    // Bridging types are only in the uniffi_jsi namespace (`ci.cpp_namespace_includes()`)
    // or the generated namespace. Most of the time, `ffi_type.cpp_namespace()` does
    // the right thing, except in the case of Callbacks and Structs.
    Ok(match ffi_type {
        FfiType::RustBuffer(_)
        | FfiType::RustCallStatus
        | FfiType::Callback(_)
        | FfiType::Struct(_) => ci.cpp_namespace(),
        FfiType::MutReference(inner) | FfiType::Reference(inner) => bridging_namespace(inner, ci)?,
        _ => ffi_type.cpp_namespace(ci),
    })
}

pub fn bridging_class(
    ffi_type: &FfiType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    let ns = bridging_namespace(ffi_type, ci)?;
    let type_name = ffi_type_name_from_js(ffi_type)?;
    Ok(format!("{ns}::Bridging<{type_name}>"))
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
        FfiType::Handle => "/*handle*/ uint64_t".into(),
        FfiType::RustBuffer(_) => "RustBuffer".into(),
        FfiType::ForeignBytes => "ForeignBytes".into(),
        FfiType::Callback(nm) => ffi_callback_name(nm)?,
        FfiType::Struct(nm) => ffi_struct_name(nm)?,
        FfiType::RustCallStatus => "RustCallStatus".into(),
        FfiType::MutReference(inner) | FfiType::Reference(inner) => {
            format!("{} *", ffi_type_name(inner)?)
        }
        FfiType::VoidPointer => "void *".into(), // ???
    })
}

fn rewrite_cpp_keywords(nm: String) -> String {
    if CPP_KEYWORDS.contains(nm.as_str()) {
        format!("{nm}_")
    } else {
        nm
    }
}

pub fn cpp_ident(nm: &str) -> Result<String, askama::Error> {
    Ok(rewrite_cpp_keywords(nm.to_string()))
}

pub fn cpp_arg_name(nm: &str) -> Result<String, askama::Error> {
    cpp_ident(nm)
}

pub fn cpp_field_name(nm: &str) -> Result<String, askama::Error> {
    cpp_ident(nm)
}

pub fn var_name(nm: &str) -> Result<String, askama::Error> {
    Ok(rewrite_cpp_keywords(nm.to_lower_camel_case()))
}

pub fn ffi_callback_name(nm: &str) -> Result<String, askama::Error> {
    Ok(format!("Uniffi{}", nm.to_upper_camel_case()))
}

pub fn ffi_struct_name(nm: &str) -> Result<String, askama::Error> {
    Ok(format!("Uniffi{}", nm.to_upper_camel_case()))
}
