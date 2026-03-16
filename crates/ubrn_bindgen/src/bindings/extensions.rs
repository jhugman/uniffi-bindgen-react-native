//! Extension traits for uniffi_bindgen types used by multiple code generators.
//!
//! Only methods needed by 2+ generators (gen_cpp, gen_typescript, gen_rust) belong here.
//! Generator-specific extensions live in their respective modules:
//! - `gen_cpp::extensions` — C++ namespace/naming, callable checks
//! - `gen_typescript::extensions` — type sorting, TS-specific queries
//! - `gen_rust::extensions` — FFI definition classification for Rust codegen

/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use extend::ext;
use uniffi_bindgen::{
    interface::{
        FfiArgument, FfiCallbackFunction, FfiFunction, FfiStruct, FfiType, Function, Method, Object,
    },
    ComponentInterface,
};

#[ext]
pub(crate) impl ComponentInterface {
    fn ffi_function_string_to_arraybuffer(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__string_to_arraybuffer".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::ForeignBytes),
            vec![FfiArgument::new("string", FfiType::RustBuffer(None))],
        );
        ffi.clone()
    }

    fn ffi_function_arraybuffer_to_string(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__arraybuffer_to_string".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::RustBuffer(None)),
            vec![FfiArgument::new("buffer", FfiType::ForeignBytes)],
        );
        ffi.clone()
    }

    fn ffi_function_string_to_bytelength(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__string_to_byte_length".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::Int32),
            vec![FfiArgument::new("string", FfiType::RustBuffer(None))],
        );
        ffi.clone()
    }

    fn iter_ffi_functions_js_to_cpp_and_back(&self) -> impl Iterator<Item = FfiFunction> {
        vec![
            self.ffi_function_string_to_bytelength(),
            self.ffi_function_string_to_arraybuffer(),
            self.ffi_function_arraybuffer_to_string(),
        ]
        .into_iter()
    }

    fn iter_ffi_functions_js_to_cpp(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_cpp_and_back()
            .chain(self.iter_ffi_functions_js_to_rust())
            .chain(self.iter_ffi_function_bless_pointer())
    }

    fn iter_ffi_functions_js_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        let has_async_calls = self.iter_callables().any(|c| c.is_async());
        self.iter_ffi_function_definitions().filter(move |f| {
            // We don't use RustBuffers directly from Typescript; we pass a Uint8Array to
            // the C++/WASM Rust, and then copy the bytes into Rust from there.
            !f.is_rustbuffer()
            // We don't want the Rust futures helper methods unless we absolutely need them.
            && (has_async_calls || !f.is_future())
        })
    }

    fn iter_ffi_function_bless_pointer(&self) -> impl Iterator<Item = FfiFunction> {
        self.object_definitions()
            .iter()
            .map(|o| o.ffi_function_bless_pointer())
    }
}

#[ext]
pub(crate) impl Object {
    fn ffi_function_bless_pointer(&self) -> FfiFunction {
        let meta = uniffi_meta::MethodMetadata {
            module_path: "internal".to_string(),
            self_name: self.name().to_string(),
            name: "ffi__bless_pointer".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
            takes_self_by_arc: false,
        };
        use uniffi_bindgen::interface::AsType;
        let receiver = self.as_type();
        let func = Method::from_metadata(meta, receiver);
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::Handle),
            vec![FfiArgument::new("pointer", FfiType::UInt64)],
        );
        ffi
    }
}

#[ext]
pub(crate) impl FfiFunction {
    fn is_future(&self) -> bool {
        self.name().contains("_rust_future_")
    }
    fn is_rustbuffer(&self) -> bool {
        self.name().contains("_rustbuffer_")
    }
}

#[ext]
pub(crate) impl FfiType {
    fn is_callable(&self) -> bool {
        matches!(self, Self::Callback(_))
    }

    fn is_void(&self) -> bool {
        matches!(self, Self::VoidPointer)
    }

    fn is_foreign_future(&self) -> bool {
        match self {
            Self::Struct(s) if s.starts_with("ForeignFuture") => true,
            Self::MutReference(t) | Self::Reference(t) => t.is_foreign_future(),
            _ => false,
        }
    }
}

#[ext]
pub(crate) impl FfiArgument {
    /// Returns true if this argument is an output parameter written by the caller.
    /// This includes both the standard return out-param (`uniffi_out_return`) and
    /// the dropped-callback out-param (`uniffi_out_dropped_callback`) introduced in
    /// uniffi 0.30 for direct-return clone callbacks.
    fn is_output_param(&self) -> bool {
        self.name() == "uniffi_out_return" || self.name() == "uniffi_out_dropped_callback"
    }
}

#[ext]
pub(crate) impl FfiCallbackFunction {
    fn is_exported(&self) -> bool {
        !self.is_user_callback() && !self.is_free_callback()
    }

    fn returns_result(&self) -> bool {
        self.is_blocking()
    }

    fn is_continuation_callback(&self) -> bool {
        is_continuation(self.name())
    }

    fn is_free_callback(&self) -> bool {
        is_free(self.name())
    }

    fn is_user_callback(&self) -> bool {
        self.name().starts_with("CallbackInterface")
    }

    fn is_function_literal(&self) -> bool {
        self.name().starts_with("ForeignFutureComplete")
    }

    fn has_return_out_param(&self) -> bool {
        self.arguments().into_iter().any(|a| a.is_output_param())
    }

    fn arg_return_type(&self) -> Option<FfiType> {
        self.arguments()
            .into_iter()
            .find(|a| a.is_output_param() && !a.type_().is_void())
            .map(|a| {
                let t = a.type_();
                match t {
                    FfiType::Reference(t) | FfiType::MutReference(t) => *t,
                    _ => t,
                }
            })
    }

    fn is_blocking(&self) -> bool {
        // If the callback returns something, or there's any chance of an error
        // (even unexpected errors), then we should block before returning
        // control back to Rust.
        // In practice this means that all user code is blocking, and uniffi internal
        // code is non-blocking: Future continuation callbacks, and free callback and
        // free future callbacks.
        self.has_return_out_param()
            || self.has_rust_call_status_arg()
            || self.return_type().is_some()
    }

    fn arguments_no_return(&self) -> impl Iterator<Item = &FfiArgument> {
        self.arguments()
            .into_iter()
            .filter(|a| !a.is_output_param())
    }
}

fn is_continuation(nm: &str) -> bool {
    nm == "RustFutureContinuationCallback"
}

fn is_free(nm: &str) -> bool {
    nm == "CallbackInterfaceFree" || nm == "ForeignFutureFree"
}

#[ext]
pub(crate) impl FfiStruct {
    fn is_exported(&self) -> bool {
        self.is_vtable() || self.is_foreign_future()
    }

    fn is_foreign_future(&self) -> bool {
        self.name().starts_with("ForeignFuture")
    }

    fn is_vtable(&self) -> bool {
        self.fields().iter().any(|f| f.type_().is_callable())
    }
}
