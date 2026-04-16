/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Type name helpers: FfiConverter names, TypeScript type labels, keyword rewriting.

use std::collections::HashSet;
use std::sync::LazyLock;

use heck::ToUpperCamelCase;

use heck::ToLowerCamelCase;
use uniffi_bindgen::pipeline::general;

pub(super) use crate::bindings::gen_typescript::type_mapping::ffi_type_to_ts as ffi_type_to_ts_name;
use crate::bindings::gen_typescript::Config;

/// Primitives use short names (`FfiConverterBool`); all others use `FfiConverter{canonical_name}`.
pub(super) fn ffi_converter_name_for(node: &general::TypeNode) -> String {
    ffi_converter_name_for_type(&node.ty)
        .unwrap_or_else(|| format!("FfiConverter{}", node.canonical_name))
}

/// Returns `None` for non-primitive types.
pub(super) fn ffi_converter_name_for_type(ty: &general::Type) -> Option<String> {
    let name = match ty {
        general::Type::Boolean => "FfiConverterBool",
        general::Type::String => "FfiConverterString",
        general::Type::Bytes => "FfiConverterArrayBuffer",
        general::Type::Int8 => "FfiConverterInt8",
        general::Type::Int16 => "FfiConverterInt16",
        general::Type::Int32 => "FfiConverterInt32",
        general::Type::Int64 => "FfiConverterInt64",
        general::Type::UInt8 => "FfiConverterUInt8",
        general::Type::UInt16 => "FfiConverterUInt16",
        general::Type::UInt32 => "FfiConverterUInt32",
        general::Type::UInt64 => "FfiConverterUInt64",
        general::Type::Float32 => "FfiConverterFloat32",
        general::Type::Float64 => "FfiConverterFloat64",
        general::Type::Timestamp => "FfiConverterTimestamp",
        general::Type::Duration => "FfiConverterDuration",
        _ => return None,
    };
    Some(name.to_string())
}

pub(super) fn type_label_for(cfg: &Config, ty: &general::Type) -> String {
    match ty {
        general::Type::UInt8 => "number".into(),
        general::Type::Int8 => "number".into(),
        general::Type::UInt16 => "number".into(),
        general::Type::Int16 => "number".into(),
        general::Type::UInt32 => "number".into(),
        general::Type::Int32 => "number".into(),
        general::Type::UInt64 => "bigint".into(),
        general::Type::Int64 => "bigint".into(),
        general::Type::Float32 => "number".into(),
        general::Type::Float64 => "number".into(),
        general::Type::Boolean => "boolean".into(),
        general::Type::String => "string".into(),
        general::Type::Bytes => {
            if cfg.strict_byte_arrays {
                "Uint8Array".into()
            } else {
                "ArrayBuffer".into()
            }
        }
        general::Type::Timestamp => "Date".into(),
        general::Type::Duration => "number".into(),
        general::Type::Interface { name, imp, .. } => {
            let name = name.to_upper_camel_case();
            if matches!(imp, general::ObjectImpl::Struct) {
                format!("{name}Like")
            } else {
                name
            }
        }
        general::Type::Record { name, .. }
        | general::Type::Enum { name, .. }
        | general::Type::CallbackInterface { name, .. }
        | general::Type::Custom { name, .. } => rewrite_js_builtins(&name.to_upper_camel_case()),
        general::Type::Optional { inner_type } => {
            format!("{} | undefined", type_label_for(cfg, inner_type))
        }
        general::Type::Sequence { inner_type } => {
            format!("Array<{}>", type_label_for(cfg, inner_type))
        }
        general::Type::Map {
            key_type,
            value_type,
        } => format!(
            "Map<{}, {}>",
            type_label_for(cfg, key_type),
            type_label_for(cfg, value_type)
        ),
    }
}

static TS_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "new",
        "null",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "as",
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
    ])
});

/// Append underscore to TypeScript reserved words.
pub(super) fn rewrite_keywords(nm: String) -> String {
    if TS_KEYWORDS.contains(nm.as_str()) {
        format!("{nm}_")
    } else {
        nm
    }
}

/// Rename types that shadow JS globals (e.g. `Error` -> `Exception`).
pub(super) fn rewrite_js_builtins(nm: &str) -> String {
    match nm {
        "Error" => "Exception".to_string(),
        _ => nm.to_string(),
    }
}

pub(super) fn fn_name(nm: &str) -> String {
    if nm == "new" {
        "create".to_string()
    } else {
        rewrite_keywords(nm.to_lower_camel_case())
    }
}

pub(super) fn arg_name(nm: &str) -> String {
    rewrite_keywords(nm.to_lower_camel_case())
}

pub(super) fn ffi_default_value_for(ffi_type: &general::FfiType) -> String {
    match ffi_type {
        general::FfiType::UInt8
        | general::FfiType::Int8
        | general::FfiType::UInt16
        | general::FfiType::Int16
        | general::FfiType::UInt32
        | general::FfiType::Int32 => "0".into(),
        general::FfiType::UInt64 | general::FfiType::Int64 | general::FfiType::Handle(_) => {
            "BigInt(0)".into()
        }
        general::FfiType::Float32 | general::FfiType::Float64 => "0.0".into(),
        general::FfiType::RustBuffer(_) => "/*empty*/ new Uint8Array(0)".into(),
        general::FfiType::Function(_) => "null".into(),
        general::FfiType::RustCallStatus => "uniffiCreateCallStatus()".into(),
        _ => String::new(),
    }
}
