use uniffi_bindgen::ComponentInterface;

use super::oracle::{CodeOracle, CodeType};

#[derive(Debug)]
pub struct CallbackInterfaceCodeType {
    id: String,
}

impl CallbackInterfaceCodeType {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl CodeType for CallbackInterfaceCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, &self.id)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.id)
    }

    fn initialization_fn(&self) -> Option<String> {
        Some(format!("uniffiCallbackInterface{}.register", self.id))
    }
}
