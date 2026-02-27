/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use extend::ext;
use heck::{ToLowerCamelCase, ToSnakeCase};
use topological_sort::TopologicalSort;
use uniffi_bindgen::{
    interface::{
        Enum, FfiArgument, FfiCallbackFunction, FfiDefinition, FfiField, FfiFunction, FfiStruct,
        FfiType, Function, Method, Object, Record, UniffiTrait,
    },
    ComponentInterface,
};
use uniffi_meta::Type;

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

    fn iter_ffi_functions_js_to_abi_rust(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_rust()
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

    fn iter_ffi_definitions_exported_by_ts(&self) -> impl Iterator<Item = FfiDefinition> {
        self.ffi_definitions().filter(|d| d.is_exported())
    }

    fn iter_ffi_callback_literals(&self) -> impl Iterator<Item = FfiCallbackFunction> {
        self.ffi_definitions().filter_map(|d| match d {
            FfiDefinition::CallbackFunction(cb) if cb.is_function_literal() => Some(cb),
            _ => None,
        })
    }

    fn iter_ffi_structs_for_callbacks(&self) -> impl Iterator<Item = FfiStruct> {
        self.ffi_definitions().filter_map(|d| match d {
            FfiDefinition::Struct(st) if st.is_foreign_future() && st.name() != "ForeignFuture" => {
                Some(st)
            }
            _ => None,
        })
    }

    fn cpp_namespace(&self) -> String {
        format!("uniffi::{}", self.namespace().to_snake_case())
    }

    fn cpp_namespace_includes(&self) -> String {
        "uniffi_jsi".to_string()
    }

    fn has_callbacks(&self) -> bool {
        !self.callback_interface_definitions().is_empty()
            || self
                .object_definitions()
                .iter()
                .any(|o| o.has_callback_interface())
    }

    fn has_async_calls(&self) -> bool {
        self.iter_callables().any(|c| c.is_async())
    }

    fn has_async_callbacks(&self) -> bool {
        self.has_async_callback_interface_definition()
    }

    /// We want to control the ordering of definitions in typescript, especially
    /// the FfiConverters which rely on Immediately Invoked Function Expressions (IIFE),
    /// or straight up expressions.
    ///
    /// These are mostly the structural types, but might also be others.
    ///
    /// The current implementations of the FfiConverters for Enums and Records do not
    /// require other FfiConverters at initialization, so we don't need to worry about
    /// ordering around their members.
    ///
    /// We can simplify code generation a little bit by including types which may
    /// not be used— e.g. UInt64 is used by object internally, but unlikely to be used by
    /// client code— we do this here, and clean up the codegen at a later stage.
    fn iter_sorted_types(&self) -> impl Iterator<Item = Type> {
        let mut graph = TopologicalSort::<String>::new();
        let mut types: HashMap<String, Type> = Default::default();
        for type_ in self.iter_local_types() {
            match type_ {
                Type::Object { name, .. } => {
                    // Objects only rely on a pointer, not the fields backing it.
                    add_edge(&mut graph, &mut types, type_, &Type::UInt64);
                    // Fields in the constructor are executed long after everything has
                    // been initialized.
                    if self.is_name_used_as_error(name) {
                        add_edge(&mut graph, &mut types, type_, &Type::Int32);
                        add_edge(&mut graph, &mut types, type_, &Type::String);
                    }
                }
                Type::Enum { name, .. } => {
                    // Ordinals are Int32.
                    add_edge(&mut graph, &mut types, type_, &Type::Int32);
                    if self.is_name_used_as_error(name) {
                        add_edge(&mut graph, &mut types, type_, &Type::String);
                    }
                }
                Type::Custom { builtin, .. } => {
                    add_edge(&mut graph, &mut types, type_, builtin.as_ref());
                }
                Type::Optional { inner_type } => {
                    add_edge(&mut graph, &mut types, type_, &Type::Boolean);
                    add_edge(&mut graph, &mut types, type_, inner_type.as_ref());
                }
                Type::Sequence { inner_type } => {
                    add_edge(&mut graph, &mut types, type_, &Type::Int32);
                    add_edge(&mut graph, &mut types, type_, inner_type.as_ref());
                }
                Type::Map {
                    key_type,
                    value_type,
                } => {
                    add_edge(&mut graph, &mut types, type_, key_type.as_ref());
                    add_edge(&mut graph, &mut types, type_, value_type.as_ref());
                }
                _ => {
                    let name = store_with_name(&mut types, type_);
                    graph.insert(name);
                }
            }
        }

        let mut sorted: Vec<Type> = Vec::new();
        while !graph.peek_all().is_empty() {
            let mut next = graph.pop_all();
            next.sort();
            sorted.extend(next.iter().filter_map(|name| types.remove(name)));
        }

        if !graph.is_empty() {
            eprintln!(
                "WARN: Cyclic dependency for typescript types: {:?}",
                types.values()
            );
            // We only warn if we have a cyclic dependency because by this stage,
            // the code generation should may be able to handle a cycle.
            // In practice, however, this will only occur if this method changes.
        }

        // I think that types should be empty by now, but we should add the remaining
        // values in to sorted, then return.
        sorted
            .into_iter()
            .chain(types.into_values())
            .filter(|t| !self.is_external(t))
    }
}

fn add_edge(
    graph: &mut TopologicalSort<String>,
    types: &mut HashMap<String, Type>,
    src: &Type,
    dest: &Type,
) {
    let src_name = store_with_name(types, src);
    let dest_name = store_with_name(types, dest);
    if src_name != dest_name {
        graph.add_dependency(dest_name, src_name);
    }
}

fn store_with_name(types: &mut HashMap<String, Type>, type_: &Type) -> String {
    let name = format!("{type_:?}");
    types.entry(name.clone()).or_insert_with(|| type_.clone());
    name
}

#[ext]
pub(crate) impl Enum {
    fn has_uniffi_traits(&self) -> bool {
        let tm = self.uniffi_trait_methods();
        tm.display_fmt.is_some()
            || tm.debug_fmt.is_some()
            || tm.eq_eq.is_some()
            || tm.hash_hash.is_some()
            || tm.ord_cmp.is_some()
    }
}

#[ext]
pub(crate) impl Record {
    /// Returns true if any Rust-defined constructor has the given name.
    /// Used to decide whether to suppress TypeScript factory helpers (`create`, `new`).
    fn has_rust_constructor_named(&self, name: &str) -> bool {
        self.constructors().iter().any(|c| c.name() == name)
    }
}

#[ext]
pub(crate) impl Object {
    fn is_uniffi_trait(t: &UniffiTrait, nm: &str) -> bool {
        match t {
            UniffiTrait::Debug { .. } => nm == "Debug",
            UniffiTrait::Display { .. } => nm == "Display",
            UniffiTrait::Eq { .. } => nm == "Eq",
            UniffiTrait::Hash { .. } => nm == "Hash",
            UniffiTrait::Ord { .. } => nm == "Ord",
        }
    }

    fn has_uniffi_trait(&self, nm: &str) -> bool {
        self.uniffi_traits()
            .iter()
            .any(|t| Self::is_uniffi_trait(t, nm))
    }

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
    fn is_internal(&self) -> bool {
        let name = self.name();
        name.contains("ffi__") && name.contains("_internal_")
    }
    fn is_callback_init(&self) -> bool {
        self.name().contains("_callback_vtable_")
    }
    fn is_future(&self) -> bool {
        self.name().contains("_rust_future_")
    }
    fn is_rustbuffer(&self) -> bool {
        self.name().contains("_rustbuffer_")
    }
}

#[ext]
pub(crate) impl FfiDefinition {
    fn is_exported(&self) -> bool {
        match self {
            Self::Function(_) => false,
            Self::CallbackFunction(cb) => cb.is_exported(),
            Self::Struct(s) => s.is_exported(),
        }
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
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        let ffi_type = FfiType::Callback(self.name().to_string());
        ffi_type.cpp_namespace(ci)
    }

    fn is_exported(&self) -> bool {
        !self.is_user_callback() && !self.is_free_callback()
    }

    fn is_rust_calling_js(&self) -> bool {
        !self.is_future_callback() || self.is_continuation_callback()
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

    fn is_clone_callback(&self) -> bool {
        self.name() == "CallbackInterfaceClone"
    }

    fn is_future_callback(&self) -> bool {
        // ForeignFutureDroppedCallback is used as a field in ForeignFutureDroppedCallbackStruct,
        // passed from JS to Rust (fromJs direction). It needs makeCallbackFunction, so it must
        // go through callback_fn_impl rather than ForeignFuture.cpp (which only generates toJs).
        self.name().starts_with("ForeignFuture") && self.name() != "ForeignFutureDroppedCallback"
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

    fn arg_return_cpp_name(&self) -> String {
        self.arguments()
            .into_iter()
            .find(|a| a.is_output_param() && !a.type_().is_void())
            .map(|a| format!("rs_{}", a.name().to_lower_camel_case()))
            .unwrap_or_else(|| "rs_uniffiOutReturn".to_string())
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
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        let ffi_type = FfiType::Struct(self.name().to_string());
        ffi_type.cpp_namespace(ci)
    }

    fn cpp_namespace_free(&self, ci: &ComponentInterface) -> String {
        format!(
            "{}::{}::free",
            self.cpp_namespace(ci),
            self.name().to_lower_camel_case().to_lowercase()
        )
    }

    fn is_exported(&self) -> bool {
        self.is_vtable() || self.is_foreign_future()
    }

    fn is_foreign_future(&self) -> bool {
        self.name().starts_with("ForeignFuture")
    }

    fn is_vtable(&self) -> bool {
        self.fields().iter().any(|f| f.type_().is_callable())
    }

    fn ffi_functions(&self) -> impl Iterator<Item = &FfiField> {
        self.fields().iter().filter(|f| f.type_().is_callable())
    }
}

#[ext]
pub(crate) impl FfiField {
    fn is_free(&self) -> bool {
        matches!(self.type_(), FfiType::Callback(s) if is_free(&s))
    }

    /// Returns true if this field is a user-defined callback interface method or clone function.
    /// These need per-vtable-field namespaces to avoid rsLambda aliasing across vtable structs.
    fn is_user_callback(&self) -> bool {
        match self.type_() {
            FfiType::Callback(name) => name.starts_with("CallbackInterface") && !is_free(&name),
            _ => false,
        }
    }

    /// Returns a namespace unique to this field within its containing vtable struct.
    /// This prevents multiple vtable structs that share the same callback type (e.g.
    /// `CallbackInterfaceClone`) from sharing a single static `rsLambda`.
    fn cpp_namespace_in_struct(&self, ci: &ComponentInterface, struct_name: &str) -> String {
        let base_ns = self.type_().cpp_namespace(ci);
        format!("{}::{}", base_ns, struct_name.to_lowercase())
    }

    /// Returns the `FfiCallbackFunction` definition for this field's callback type,
    /// or `None` if the field is not a `Callback` type.
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
}
