/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use uniffi_bindgen::{interface::Type, ComponentInterface};
use uniffi_meta::AsType;

#[derive(Default, Debug)]
struct CiTypeMap {
    types: HashMap<String, Type>,
}

#[derive(Default, Debug)]
pub(crate) struct TypeMap {
    modules: HashMap<String, CiTypeMap>,
}

impl TypeMap {
    pub(crate) fn insert_type(&mut self, type_: Type) {
        let (module_path, name) = match &type_ {
            Type::CallbackInterface { module_path, name }
            | Type::Enum { module_path, name }
            | Type::Object {
                module_path, name, ..
            }
            | Type::Record { module_path, name }
            | Type::Custom {
                module_path, name, ..
            } => (module_path, name),
            _ => return,
        };
        let module = self.modules.entry(module_path.clone()).or_default();
        module.types.insert(name.clone(), type_);
    }

    pub(crate) fn insert_ci(&mut self, ci: &ComponentInterface) {
        for type_ in ci.iter_types() {
            self.insert_type(type_.clone());
        }
    }

    pub(crate) fn as_type(&self, as_type: &impl AsType) -> Type {
        let t = as_type.as_type();
        match t {
            Type::External {
                module_path, name, ..
            } => {
                let module = self.modules.get(&module_path).expect("module not found");
                let type_ = module.types.get(&name).expect("type not found");
                type_.clone()
            }
            Type::Optional { inner_type } => Type::Optional {
                inner_type: Box::new(self.as_type(&inner_type)),
            },
            Type::Sequence { inner_type } => Type::Sequence {
                inner_type: Box::new(self.as_type(&inner_type)),
            },
            Type::Map {
                key_type,
                value_type,
            } => Type::Map {
                key_type: Box::new(self.as_type(&key_type)),
                value_type: Box::new(self.as_type(&value_type)),
            },
            _ => t,
        }
    }
}
