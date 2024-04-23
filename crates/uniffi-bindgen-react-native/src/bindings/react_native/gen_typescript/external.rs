use super::oracle::{CodeOracle, CodeType};
use uniffi_bindgen::ComponentInterface;

#[derive(Debug)]
pub struct ExternalCodeType {
    name: String,
}

impl ExternalCodeType {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl CodeType for ExternalCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, &self.name)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.name)
    }
}
