use uniffi_bindgen::{backend::Literal, ComponentInterface};

use super::oracle::{CodeOracle, CodeType};

#[derive(Debug)]
pub struct EnumCodeType {
    id: String,
}

impl EnumCodeType {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl CodeType for EnumCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, &self.id)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.id)
    }

    fn literal(&self, literal: &Literal, ci: &ComponentInterface) -> String {
        if let Literal::Enum(v, _) = literal {
            format!(
                "{}.{}",
                self.type_label(ci),
                CodeOracle.enum_variant_name(v)
            )
        } else {
            unreachable!();
        }
    }
}
