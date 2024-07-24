/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::oracle::{AsCodeType, CodeOracle};
pub(crate) use uniffi_bindgen::backend::filters::*;
use uniffi_bindgen::{
    backend::{filters::UniFFIError, Literal, Type},
    interface::{AsType, Enum, FfiType, Object, Variant},
    ComponentInterface,
};

pub(super) fn type_name(
    as_ct: &impl AsCodeType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().type_label(ci))
}

pub(super) fn decl_type_name(
    as_ct: &impl AsCodeType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().decl_type_label(ci))
}

pub(super) fn canonical_name(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().canonical_name())
}

pub(super) fn ffi_converter_name(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().ffi_converter_name())
}

#[allow(unused)]
pub(super) fn ffi_error_converter_name(as_type: &impl AsType) -> Result<String, askama::Error> {
    // special handling for types used as errors.
    let mut name = ffi_converter_name(as_type)?;
    if matches!(&as_type.as_type(), Type::Object { .. }) {
        name.push_str("__as_error")
    }
    Ok(name)
}

pub(super) fn lower_fn(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(format!(
        "{ct}.lower.bind({ct})",
        ct = as_ct.as_codetype().ffi_converter_name()
    ))
}

#[allow(unused)]
pub(super) fn allocation_size_fn(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(format!(
        "{}.allocationSize",
        as_ct.as_codetype().ffi_converter_name()
    ))
}

pub(super) fn write_fn(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(format!(
        "{}.write",
        as_ct.as_codetype().ffi_converter_name()
    ))
}

pub(super) fn lift_fn(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(format!(
        "{ct}.lift.bind({ct})",
        ct = as_ct.as_codetype().ffi_converter_name()
    ))
}

pub(super) fn read_fn(as_ct: &impl AsCodeType) -> Result<String, askama::Error> {
    Ok(format!("{}.read", as_ct.as_codetype().ffi_converter_name()))
}

pub fn render_literal(
    literal: &Literal,
    as_ct: &impl AsType,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    Ok(as_ct.as_codetype().literal(literal, ci))
}

// Get the idiomatic Typescript rendering of an integer.
#[allow(unused)]
fn int_literal(t: &Option<Type>, base10: String) -> Result<String, askama::Error> {
    if let Some(t) = t {
        match t {
            Type::Int8 | Type::Int16 | Type::Int32 | Type::Int64 => Ok(base10),
            Type::UInt8 | Type::UInt16 | Type::UInt32 | Type::UInt64 => Ok(base10 + "u"),
            _ => Err(askama::Error::Custom(Box::new(UniFFIError::new(
                "Only ints are supported.".to_string(),
            )))),
        }
    } else {
        Err(askama::Error::Custom(Box::new(UniFFIError::new(
            "Enum hasn't defined a repr".to_string(),
        ))))
    }
}

/// Get the idiomatic Python rendering of an individual enum variant.
pub fn object_names(
    obj: &Object,
    ci: &ComponentInterface,
) -> Result<(String, String), askama::Error> {
    Ok(CodeOracle.object_names(ci, obj))
}

pub fn variant_discr_literal(
    e: &Enum,
    index: &usize,
    ci: &ComponentInterface,
) -> Result<String, askama::Error> {
    let literal = e.variant_discr(*index).expect("invalid index");
    Ok(Type::Int32.as_codetype().literal(&literal, ci))
}

pub fn ffi_type_name_for_cpp(type_: &FfiType, is_internal: &bool) -> Result<String, askama::Error> {
    Ok(if *is_internal {
        CodeOracle.ffi_type_label_for_cpp(type_)
    } else {
        CodeOracle.ffi_type_label(type_)
    })
}

#[allow(unused)]
pub fn ffi_type_name(ffi_type: &FfiType) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_type_label(ffi_type))
}

#[allow(unused)]
pub fn ffi_type_name_for_ffi_struct(type_: &FfiType) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_type_label_for_ffi_struct(type_))
}

#[allow(unused)]
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
#[allow(unused)]
pub fn ffi_callback_name(nm: &str) -> Result<String, askama::Error> {
    Ok(CodeOracle.ffi_callback_name(nm))
}

/// Get the idiomatic Typescript rendering of an FFI struct name
#[allow(unused)]
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
