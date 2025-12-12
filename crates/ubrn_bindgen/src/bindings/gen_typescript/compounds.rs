/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::oracle::{AsCodeType, CodeOracle, CodeType};
use uniffi_bindgen::{
    interface::{Literal, Type},
    ComponentInterface,
};

#[derive(Debug)]
pub struct OptionalCodeType {
    inner: Type,
}

impl OptionalCodeType {
    pub fn new(inner: Type) -> Self {
        Self { inner }
    }
    fn inner(&self) -> &Type {
        &self.inner
    }
}

impl CodeType for OptionalCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        let inner = self.inner();
        let inner_ts = CodeOracle.find(inner).type_label(ci);
        if !matches!(inner, Type::Optional { .. }) {
            format!("{inner_ts} | undefined",)
        } else {
            // Nested optionals shouldn't degenerate into T | undefined | undefined
            inner_ts
        }
    }

    fn canonical_name(&self) -> String {
        format!("Optional{}", CodeOracle.find(self.inner()).canonical_name())
    }

    fn literal(&self, literal: &Literal, ci: &ComponentInterface) -> String {
        match literal {
            Literal::None => "undefined".into(),
            Literal::Some { inner } => {
                // In UniFFI 0.30, Some contains a DefaultValue (DefaultValueMetadata)
                match &**inner {
                    DefaultValue::Literal(inner_literal) => {
                        // Recursively render the inner literal value
                        CodeOracle.find(self.inner()).literal(inner_literal, ci)
                    }
                    DefaultValue::Default => {
                        // For Some(Default), we need to provide the default value of the inner type.
                        // For primitive types, this is well-defined (0, false, "", etc.)
                        // For complex types (enums, records), we generate a placeholder comment.
                        let inner_type = self.inner();
                        let code_type = CodeOracle.find(inner_type);
                        match inner_type {
                            Type::String => "\"\"".into(),
                            Type::Boolean => "false".into(),
                            Type::Int8 | Type::Int16 | Type::Int32 => "0".into(),
                            Type::Int64 => "0n".into(),
                            Type::UInt8 | Type::UInt16 | Type::UInt32 => "0".into(),
                            Type::UInt64 => "0n".into(),
                            Type::Float32 | Type::Float64 => "0.0".into(),
                            _ => format!("/* default for {} */", code_type.type_label(ci)),
                        }
                    }
                }
            },
            _ => panic!("Invalid literal for Optional type: {literal:?}"),
        }
    }
}

#[derive(Debug)]
pub struct SequenceCodeType {
    inner: Type,
}

impl SequenceCodeType {
    pub fn new(inner: Type) -> Self {
        Self { inner }
    }
    fn inner(&self) -> &Type {
        &self.inner
    }
}

impl CodeType for SequenceCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        format!("Array<{}>", CodeOracle.find(self.inner()).type_label(ci))
    }

    fn canonical_name(&self) -> String {
        format!("Array{}", CodeOracle.find(self.inner()).canonical_name())
    }

    fn literal(&self, literal: &Literal, _ci: &ComponentInterface) -> String {
        match literal {
            Literal::EmptySequence => "[]".into(),
            _ => panic!("Invalid literal for List type: {literal:?}"),
        }
    }
}

#[derive(Debug)]
pub struct MapCodeType {
    key: Type,
    value: Type,
}

impl MapCodeType {
    pub fn new(key: Type, value: Type) -> Self {
        Self { key, value }
    }

    fn key(&self) -> &Type {
        &self.key
    }

    fn value(&self) -> &Type {
        &self.value
    }
}

impl CodeType for MapCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        format!(
            "Map<{}, {}>",
            CodeOracle.find(self.key()).type_label(ci),
            CodeOracle.find(self.value()).type_label(ci),
        )
    }

    fn canonical_name(&self) -> String {
        format!(
            "Map{}{}",
            self.key().as_codetype().canonical_name(),
            self.value().as_codetype().canonical_name(),
        )
    }

    fn literal(&self, literal: &Literal, _ci: &ComponentInterface) -> String {
        match literal {
            Literal::EmptyMap => "mapOf()".into(),
            _ => panic!("Invalid literal for Map type: {literal:?}"),
        }
    }
}
