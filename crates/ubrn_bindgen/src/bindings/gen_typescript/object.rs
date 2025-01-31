/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
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
    // This is the name of the type/interface.
    //
    // In Typescript, this is `{class_name}Interface`, and for callback
    // interfaces (when `self.imp.has_callback_interface()`) `{class_name}`.
    //
    fn type_label(&self, ci: &ComponentInterface) -> String {
        if !self.imp.is_trait_interface() || ci.is_name_used_as_error(&self.name) {
            format!("{}Interface", CodeOracle.class_name(ci, &self.name))
        } else {
            CodeOracle.class_name(ci, &self.name)
        }
    }

    // This is the name of the implementation class, that implements the interface
    // above.
    //
    // In Typescript, this is `{class_name}`, and for callback
    // interfaces (when `self.imp.has_callback_interface()`) `{class_name}Impl`.
    //
    // Unlike other languages, in Typescript it is legal to have the interface/type called the same thing
    // as the implementation class. This is very useful so as avoid extra cognitive burden,
    // and naming collisions.
    fn decl_type_label(&self, ci: &ComponentInterface) -> String {
        if !self.imp.is_trait_interface() || ci.is_name_used_as_error(&self.name) {
            CodeOracle.class_name(ci, &self.name)
        } else {
            format!("{}Impl", CodeOracle.class_name(ci, &self.name))
        }
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
