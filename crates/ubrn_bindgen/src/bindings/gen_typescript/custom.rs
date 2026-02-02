/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
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
    fn type_label(&self, ci: &ComponentInterface, _opt_out_interface: bool) -> String {
        CodeOracle.class_name(ci, &self.name)
    }

    fn canonical_name(&self) -> String {
        format!("Type{}", self.name)
    }
}
