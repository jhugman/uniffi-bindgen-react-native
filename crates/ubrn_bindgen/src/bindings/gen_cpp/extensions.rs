/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use extend::ext;
use heck::{ToLowerCamelCase, ToSnakeCase};
use uniffi_bindgen::{
    interface::{FfiCallbackFunction, FfiDefinition, FfiField, FfiFunction, FfiStruct, FfiType},
    ComponentInterface,
};

use crate::bindings::extensions::{
    FfiArgumentExt as _, FfiCallbackFunctionExt as _, FfiStructExt as _, FfiTypeExt as _,
};

#[ext(name = CppComponentInterfaceExt)]
pub(super) impl ComponentInterface {
    fn cpp_namespace(&self) -> String {
        format!("uniffi::{}", self.namespace().to_snake_case())
    }

    fn cpp_namespace_includes(&self) -> String {
        "uniffi_jsi".to_string()
    }

    fn iter_ffi_structs(&self) -> impl Iterator<Item = FfiStruct> {
        self.ffi_definitions().filter_map(|s| match s {
            FfiDefinition::Struct(s) => Some(s),
            _ => None,
        })
    }

    fn iter_ffi_structs_for_free(&self) -> impl Iterator<Item = FfiStruct> {
        self.iter_ffi_structs()
            .filter(|s| !s.is_foreign_future() || s.name() == "ForeignFuture")
    }
}

#[ext(name = CppFfiFunctionExt)]
pub(super) impl FfiFunction {
    fn is_callback_init(&self) -> bool {
        self.name().contains("_callback_vtable_")
    }
}

#[ext(name = CppFfiTypeExt)]
pub(super) impl FfiType {
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        match self {
            Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Float32
            | Self::Float64
            | Self::Handle
            | Self::RustCallStatus
            | Self::RustBuffer(_)
            | Self::VoidPointer => ci.cpp_namespace_includes(),
            Self::Callback(name) => format!(
                "{}::cb::{}",
                ci.cpp_namespace(),
                name.to_lower_camel_case().to_lowercase()
            ),
            Self::Struct(name) => format!(
                "{}::st::{}",
                ci.cpp_namespace(),
                name.to_lower_camel_case().to_lowercase()
            ),
            _ => ci.cpp_namespace(),
        }
    }
}

#[ext(name = CppFfiCallbackFunctionExt)]
pub(super) impl FfiCallbackFunction {
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        FfiType::Callback(self.name().to_string()).cpp_namespace(ci)
    }

    fn is_future_callback(&self) -> bool {
        // ForeignFutureDroppedCallback is used as a field in ForeignFutureDroppedCallbackStruct,
        // passed from JS to Rust (fromJs direction). It needs makeCallbackFunction, so it must
        // go through callback_fn_impl rather than ForeignFuture.cpp (which only generates toJs).
        self.name().starts_with("ForeignFuture") && self.name() != "ForeignFutureDroppedCallback"
    }

    fn is_rust_calling_js(&self) -> bool {
        !self.is_future_callback() || self.is_continuation_callback()
    }

    fn arg_return_cpp_name(&self) -> String {
        self.arguments()
            .into_iter()
            .find(|a| a.is_output_param() && !a.type_().is_void())
            .map(|a| format!("rs_{}", a.name().to_lower_camel_case()))
            .unwrap_or_else(|| "rs_uniffiOutReturn".to_string())
    }
}

#[ext(name = CppFfiStructExt)]
pub(super) impl FfiStruct {
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        FfiType::Struct(self.name().to_string()).cpp_namespace(ci)
    }

    fn cpp_namespace_free(&self, ci: &ComponentInterface) -> String {
        format!(
            "{}::{}::free",
            self.cpp_namespace(ci),
            self.name().to_lower_camel_case().to_lowercase()
        )
    }

    fn ffi_functions(&self) -> impl Iterator<Item = &FfiField> {
        self.fields().iter().filter(|f| f.type_().is_callable())
    }
}

#[ext(name = CppFfiFieldExt)]
pub(super) impl FfiField {
    fn cpp_namespace_in_struct(&self, ci: &ComponentInterface, struct_name: &str) -> String {
        let base_ns = self.type_().cpp_namespace(ci);
        format!("{}::{}", base_ns, struct_name.to_lowercase())
    }

    fn callback_function(&self, ci: &ComponentInterface) -> Option<FfiCallbackFunction> {
        if let FfiType::Callback(name) = self.type_() {
            for def in ci.ffi_definitions() {
                if let FfiDefinition::CallbackFunction(cb) = def {
                    if cb.name() == name {
                        return Some(cb);
                    }
                }
            }
        }
        None
    }

    fn is_free(&self) -> bool {
        matches!(self.type_(), FfiType::Callback(s) if s == "CallbackInterfaceFree" || s == "ForeignFutureFree")
    }

    /// Returns true if this field is a user-defined callback interface method or clone function.
    /// These need per-vtable-field namespaces to avoid rsLambda aliasing across vtable structs.
    fn is_user_callback(&self) -> bool {
        match self.type_() {
            FfiType::Callback(name) => {
                name.starts_with("CallbackInterface") && name != "CallbackInterfaceFree"
            }
            _ => false,
        }
    }
}
