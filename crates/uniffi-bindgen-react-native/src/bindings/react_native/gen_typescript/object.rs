use super::oracle::{CodeOracle, CodeType};
use uniffi_bindgen::{interface::ObjectImpl, ComponentInterface};

#[derive(Debug)]
pub struct ObjectCodeType {
    name: String,
    imp: ObjectImpl,
}

impl ObjectCodeType {
    pub fn new(name: String, imp: ObjectImpl) -> Self {
        Self { name, imp }
    }
}

impl CodeType for ObjectCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, &self.name)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.name)
    }

    fn initialization_fn(&self) -> Option<String> {
        self.imp
            .has_callback_interface()
            .then(|| format!("uniffiCallbackInterface{}.register", self.name))
    }
}
