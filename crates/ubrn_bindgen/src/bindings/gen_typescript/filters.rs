/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::{
    oracle::{AsCodeType, CodeOracle},
    TypeRenderer,
};
pub(crate) use uniffi_bindgen::backend::filters::*;
use uniffi_bindgen::{
    backend::{Literal, Type},
    interface::{AsType, Enum, FfiType, Variant},
    ComponentInterface,
};

pub(super) fn type_name(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    let type_ = types.as_type(as_type);
    Ok(type_.as_codetype().type_label(types.ci))
}

pub(super) fn ffi_type_name_from_type(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    let type_ = types.as_type(as_type);
    let ffi_type = FfiType::from(type_);
    ffi_type_name(&ffi_type)
}

pub(super) fn decl_type_name(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    let type_ = types.as_type(as_type);
    Ok(type_.as_codetype().decl_type_label(types.ci))
}

pub(super) fn ffi_converter_name(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    let type_ = types.as_type(as_type);
    Ok(type_.as_codetype().ffi_converter_name())
}

pub(super) fn ffi_error_converter_name(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    // special handling for types used as errors.
    let type_ = types.as_type(as_type);
    let mut name = type_.as_codetype().ffi_converter_name();
    if matches!(type_, Type::Object { .. }) {
        name.push_str("__as_error")
    }
    if types.ci.is_external(&type_) {
        let module_path = type_
            .module_path()
            .expect("External type should have a module path");
        let namespace = types
            .ci
            .namespace_for_module_path(module_path)
            .expect("Module path should map to namespace");
        types.import_converter(name.clone(), namespace);
    }
    Ok(name)
}

pub(super) fn lower_error_fn(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    Ok(format!(
        "{ct}.lower.bind({ct})",
        ct = ffi_error_converter_name(as_type, types)?
    ))
}

pub(super) fn lift_error_fn(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    Ok(format!(
        "{ct}.lift.bind({ct})",
        ct = ffi_error_converter_name(as_type, types)?
    ))
}

pub(super) fn lift_fn(
    as_type: &impl AsType,
    types: &TypeRenderer,
) -> Result<String, askama::Error> {
    Ok(format!(
        "{ct}.lift.bind({ct})",
        ct = ffi_converter_name(as_type, types)?
    ))
}

pub fn render_literal(
    literal: &Literal,
    as_ct: &impl AsType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().literal(literal, ci))
}

pub fn variant_discr_literal(
    e: &Enum,
    index: &usize,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    let literal = e.variant_discr(*index).expect("invalid index");
    let ts_literal = Type::Int32.as_codetype().literal(&literal, ci);
    Ok(match literal {
        Literal::String(_) => ts_literal,
        Literal::UInt(_, _, typ) | Literal::Int(_, _, typ)
            if !matches!(&typ, &Type::Int64 | &Type::UInt64) =>
        {
            ts_literal
        }
        // Discriminant do not travel across the FFI, so we don't have to maintain bit
        // parity here: we can do things for the convenience of the ts developer.
        // Here, we cast a BigInt down to a number iff the number is representable by the number.
        Literal::UInt(n, _, _) => {
            if n < (u32::MAX as u64) {
                format!("{n}")
            } else {
                format!("\"{n}\"")
            }
        }
        Literal::Int(n, _, _) => {
            if (i32::MIN as i64) < n && n < (i32::MAX as i64) {
                format!("{n}")
            } else {
                format!("\"{n}\"")
            }
        }
        _ => format!("\"{ts_literal}\""),
    })
}

pub fn ffi_type_name_for_cpp(type_: &FfiType, is_internal: &bool) -> Result<String, askama::Error> {
    Ok(if *is_internal {
        CodeOracle.ffi_type_label_for_cpp(type_)
    } else {
        CodeOracle.ffi_type_label(type_)
    })
}

pub fn ffi_type_name(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_type_label(ffi_type))
}

pub fn ffi_default_value(type_: &FfiType) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_default_value(type_))
}

/// Get the idiomatic Typescript rendering of a function name.
pub fn class_name(nm: &str, ci: &ComponentInterface) -> Result<String, askama::Error> {
    Ok(CodeOracle.class_name(ci, nm))
}

/// Get the idiomatic Typescript rendering of a function name.
pub fn fn_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.fn_name(nm))
}

/// Get the idiomatic Typescript rendering of a variable name.
pub fn var_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.var_name(nm))
}

/// Get the idiomatic Swift rendering of an arguments name.
/// This is the same as the var name but quoting is not required.
pub fn arg_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.var_name(nm))
}

/// Get a String representing the name used for an individual enum variant.
pub fn variant_name(v: &Variant) -> Result<String, askama::Error> {
    Ok(CodeOracle.enum_variant_name(v.name()))
}

/// Get the idiomatic Typescript rendering of an FFI callback function name
pub fn ffi_callback_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_callback_name(nm))
}

/// Get the idiomatic Typescript rendering of an FFI struct name
pub fn ffi_struct_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_struct_name(nm))
}

/// Get the idiomatic Typescript rendering of docstring
pub fn docstring(docstring: &str, spaces: &i32) -> Result<String, askama::Error> {
    let middle = textwrap::indent(&textwrap::dedent(docstring), " * ");
    let wrapped = format!("/**\n{middle}\n */");

    let spaces = usize::try_from(*spaces).unwrap_or_default();
    Ok(textwrap::indent(&wrapped, &" ".repeat(spaces)))
}
