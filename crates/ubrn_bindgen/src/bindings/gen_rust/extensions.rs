/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use extend::ext;
use heck::ToSnakeCase;
use syn::Ident;
use uniffi_bindgen::{
    interface::{FfiArgument, FfiCallbackFunction, FfiDefinition, FfiField, FfiStruct, FfiType},
    ComponentInterface,
};

use super::{ident, snake_case_ident};
use crate::bindings::extensions::{FfiCallbackFunctionExt as _, FfiStructExt as _};

#[ext]
pub(super) impl ComponentInterface {
    // This is going to be very difficult to test as FfiStruct and FfiCallbackFunction
    // aren't easily constructable.
    fn ffi_definitions2(&self) -> impl Iterator<Item = FfiDefinition2> {
        let has_async_callbacks = self.has_async_callback_interface_definition();
        let has_async_calls = self.iter_callables().any(|c| c.is_async());
        ffi_definitions2(self.ffi_definitions(), has_async_calls, has_async_callbacks)
    }
}

fn ffi_definitions2(
    definitions: impl Iterator<Item = FfiDefinition>,
    has_async_calls: bool,
    has_async_callbacks: bool,
) -> impl Iterator<Item = FfiDefinition2> {
    let mut callbacks = HashMap::new();
    let mut structs = HashMap::new();
    for definition in definitions {
        match definition {
            FfiDefinition::CallbackFunction(cb) => {
                callbacks.insert(cb.name().to_owned(), cb);
            }
            FfiDefinition::Struct(st) => {
                structs.insert(st.name().to_owned(), st);
            }
            _ => (),
        }
    }
    let mut definitions = Vec::new();
    for ffi_struct in structs.into_values() {
        if !has_async_callbacks && ffi_struct.is_foreign_future() {
            // we will do something different with future callbacks.
            continue;
        }
        let mut method_module_idents = HashMap::new();
        for field in ffi_struct.fields() {
            let FfiType::Callback(name) = &field.type_() else {
                continue;
            };
            let Some(callback) = callbacks.get(name) else {
                panic!("Missing callback. This is a bug in ubrn");
            };
            let module_ident = if callback.is_free_callback() {
                let ident = callback.module_ident_free(&ffi_struct);
                let callback = callback.clone();
                let module_ident = ident.clone();
                let cb = FfiCallbackFunction2 {
                    callback,
                    module_ident,
                };
                definitions.push(FfiDefinition2::CallbackFunction(cb));
                ident
            } else {
                callback.module_ident()
            };
            method_module_idents.insert(field.name().to_string(), module_ident);
        }
        definitions.push(FfiDefinition2::Struct(FfiStruct2 {
            ffi_struct,
            methods: method_module_idents,
        }));
    }

    for callback in callbacks.into_values() {
        if callback.is_free_callback() {
            // this is done above.
            continue;
        }
        if !has_async_callbacks && callback.is_future_callback() {
            // We don't need to do anything if we have no async callbacks.
            continue;
        }
        if !has_async_calls && callback.is_continuation_callback() {
            // We don't need to do anything if we have no async functions or methods.
            continue;
        }
        let cb = FfiCallbackFunction2 {
            module_ident: callback.module_ident(),
            callback,
        };
        definitions.push(FfiDefinition2::CallbackFunction(cb));
    }
    definitions.into_iter()
}

#[ext]
impl FfiArgument {
    fn is_return(&self) -> bool {
        self.name() == "uniffi_out_return"
    }
}

pub(super) enum FfiDefinition2 {
    CallbackFunction(FfiCallbackFunction2),
    Struct(FfiStruct2),
}

pub(super) struct FfiCallbackFunction2 {
    module_ident: Ident,
    callback: FfiCallbackFunction,
}

pub(super) struct FfiStruct2 {
    ffi_struct: FfiStruct,
    methods: HashMap<String, Ident>,
}

#[ext]
pub(super) impl FfiStruct {
    fn module_ident(&self) -> Ident {
        snake_case_ident(self.name())
    }
}

#[ext]
pub(super) impl FfiCallbackFunction {
    fn module_ident_free(&self, enclosing: &FfiStruct) -> Ident {
        ident(&format!("{}__free", enclosing.name().to_snake_case()))
    }
    fn module_ident(&self) -> Ident {
        snake_case_ident(self.name())
    }
}

impl FfiCallbackFunction2 {
    pub(super) fn module_ident(&self) -> Ident {
        self.module_ident.clone()
    }
    pub(super) fn return_type(&self) -> Option<FfiType> {
        self.callback.arg_return_type()
    }
    pub(super) fn has_return_out_param(&self) -> bool {
        self.callback.has_return_out_param()
    }
    pub(super) fn callback(&self) -> &FfiCallbackFunction {
        &self.callback
    }
}

impl FfiStruct2 {
    pub(super) fn module_ident(&self) -> Ident {
        self.ffi_struct.module_ident()
    }
    pub(super) fn is_callback_method(&self, name: &str) -> bool {
        self.methods.contains_key(name)
    }
    pub(super) fn method_alias_ident(&self, name: &str) -> Ident {
        ident(&format!("method_{name}"))
    }
    pub(super) fn method_mod_ident(&self, name: &str) -> Ident {
        self.methods
            .get(name)
            .expect("Method not found. This is probably a ubrn bug.")
            .clone()
    }

    /// An iterator of method names, in the order that they are declared.
    pub(super) fn method_names(&self) -> impl Iterator<Item = &str> {
        self.ffi_struct
            .fields()
            .iter()
            .filter(|f| matches!(f.type_(), FfiType::Callback(_)))
            .map(|f| f.name())
    }

    pub(super) fn fields(&self) -> impl Iterator<Item = &FfiField> {
        self.ffi_struct.fields().iter()
    }
}
