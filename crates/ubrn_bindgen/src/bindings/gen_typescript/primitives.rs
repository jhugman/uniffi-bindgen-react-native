/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::oracle::CodeType;
use paste::paste;
use uniffi_bindgen::{
    interface::{Literal, Type},
    interface::Radix,
    ComponentInterface,
};

fn render_literal(literal: &Literal, _ci: &ComponentInterface) -> String {
    fn typed_number(type_: &Type, num_str: String) -> String {
        let unwrapped_type = match type_ {
            Type::Optional { inner_type } => inner_type,
            t => t,
        };
        match unwrapped_type {
            // Bytes, Shorts and Ints can all be inferred from the type.
            Type::Int8 | Type::Int16 | Type::Int32 => num_str,
            Type::Int64 => format!("BigInt(\"{num_str}\")"),

            Type::UInt8 | Type::UInt16 | Type::UInt32 => num_str,
            Type::UInt64 => format!("BigInt(\"{num_str}\")"),

            Type::Float32 | Type::Float64 => num_str,
            _ => panic!("Unexpected literal: {num_str} for type: {type_:?}"),
        }
    }

    match literal {
        Literal::Boolean(v) => format!("{v}"),
        Literal::String(s) => format!("\"{s}\""),
        Literal::Int(i, radix, type_) => typed_number(
            type_,
            match radix {
                Radix::Octal => format!("{i:#x}"),
                Radix::Decimal => format!("{i}"),
                Radix::Hexadecimal => format!("{i:#x}"),
            },
        ),
        Literal::UInt(i, radix, type_) => typed_number(
            type_,
            match radix {
                Radix::Octal => format!("{i:#x}"),
                Radix::Decimal => format!("{i}"),
                Radix::Hexadecimal => format!("{i:#x}"),
            },
        ),
        Literal::Float(string, type_) => typed_number(type_, string.clone()),

        _ => unreachable!("Literal"),
    }
}

macro_rules! impl_code_type_for_primitive {
    ($T:ty, $canonical_name:literal, $class_name:literal) => {
        paste! {
            #[derive(Debug)]
            pub struct $T;

            impl CodeType for $T  {
                fn type_label(&self, _ci: &ComponentInterface) -> String {
                    $class_name.into()
                }

                fn canonical_name(&self) -> String {
                    $canonical_name.into()
                }

                fn literal(&self, literal: &Literal, ci: &ComponentInterface) -> String {
                    render_literal(&literal, ci)
                }
            }
        }
    };
}

impl_code_type_for_primitive!(BooleanCodeType, "Bool", "boolean");
impl_code_type_for_primitive!(StringCodeType, "String", "string");
impl_code_type_for_primitive!(BytesCodeType, "ArrayBuffer", "ArrayBuffer");
impl_code_type_for_primitive!(Int8CodeType, "Int8", "/*i8*/number");
impl_code_type_for_primitive!(Int16CodeType, "Int16", "/*i16*/number");
impl_code_type_for_primitive!(Int32CodeType, "Int32", "/*i32*/number");
impl_code_type_for_primitive!(Int64CodeType, "Int64", "/*i64*/bigint");
impl_code_type_for_primitive!(UInt8CodeType, "UInt8", "/*u8*/number");
impl_code_type_for_primitive!(UInt16CodeType, "UInt16", "/*u16*/number");
impl_code_type_for_primitive!(UInt32CodeType, "UInt32", "/*u32*/number");
impl_code_type_for_primitive!(UInt64CodeType, "UInt64", "/*u64*/bigint");
impl_code_type_for_primitive!(Float32CodeType, "Float32", "/*f32*/number");
impl_code_type_for_primitive!(Float64CodeType, "Float64", "/*f64*/number");
