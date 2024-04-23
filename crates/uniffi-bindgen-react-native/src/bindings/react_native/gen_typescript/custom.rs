use super::oracle::{CodeOracle, CodeType};
use uniffi_bindgen::ComponentInterface;

#[derive(Debug)]
pub struct CustomCodeType {
    name: String,
}

impl CustomCodeType {
    pub fn new(name: String) -> Self {
        CustomCodeType { name }
    }
}

impl CodeType for CustomCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, &self.name)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.name)
    }
}
