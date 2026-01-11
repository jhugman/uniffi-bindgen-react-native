/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::oracle::{AsCodeType, CodeOracle, CodeType};
use anyhow::{bail, Result};
use uniffi_bindgen::{
    interface::{DefaultValue, Literal, Type},
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

    fn default(&self, default: &DefaultValue, ci: &ComponentInterface) -> Result<String> {
        match default {
            DefaultValue::Default | DefaultValue::Literal(Literal::None) => Ok("undefined".into()),
            DefaultValue::Literal(Literal::Some { inner }) => {
                CodeOracle.find(&self.inner).default(inner, ci)
            }
            _ => bail!("Invalid literal for Optional type: {default:?}"),
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

    fn default(&self, default: &DefaultValue, _ci: &ComponentInterface) -> Result<String> {
        match default {
            DefaultValue::Default | DefaultValue::Literal(Literal::EmptySequence) => {
                Ok("[]".into())
            }
            _ => bail!("Invalid literal for List type: {default:?}"),
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

    fn default(&self, default: &DefaultValue, _ci: &ComponentInterface) -> Result<String> {
        match default {
            DefaultValue::Default | DefaultValue::Literal(Literal::EmptyMap) => {
                Ok("new Map()".into())
            }
            _ => bail!("Invalid literal for Map type: {default:?}"),
        }
    }
}
