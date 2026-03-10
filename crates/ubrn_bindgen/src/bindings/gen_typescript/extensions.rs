/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::collections::HashMap;

use extend::ext;
use topological_sort::TopologicalSort;
use uniffi_bindgen::{
    interface::{
        Enum, FfiCallbackFunction, FfiDefinition, FfiFunction, FfiStruct, Object, Record,
        UniffiTrait,
    },
    ComponentInterface,
};
use uniffi_meta::Type;

use crate::bindings::extensions::{FfiCallbackFunctionExt as _, FfiStructExt as _};

#[ext(name = TsFfiFunctionExt)]
pub(super) impl FfiFunction {
    fn is_internal(&self) -> bool {
        let name = self.name();
        name.contains("ffi__") && name.contains("_internal_")
    }
}

#[ext(name = TsComponentInterfaceExt)]
pub(super) impl ComponentInterface {
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

#[ext(name = TsFfiDefinitionExt)]
pub(super) impl FfiDefinition {
    fn is_exported(&self) -> bool {
        match self {
            Self::Function(_) => false,
            Self::CallbackFunction(cb) => cb.is_exported(),
            Self::Struct(s) => s.is_exported(),
        }
    }
}

#[ext(name = TsEnumExt)]
pub(super) impl Enum {
    fn has_uniffi_traits(&self) -> bool {
        let tm = self.uniffi_trait_methods();
        tm.display_fmt.is_some()
            || tm.debug_fmt.is_some()
            || tm.eq_eq.is_some()
            || tm.hash_hash.is_some()
            || tm.ord_cmp.is_some()
    }
}

#[ext(name = TsRecordExt)]
pub(super) impl Record {
    /// Returns true if any Rust-defined constructor has the given name.
    /// Used to decide whether to suppress TypeScript factory helpers (`create`, `new`).
    fn has_rust_constructor_named(&self, name: &str) -> bool {
        self.constructors().iter().any(|c| c.name() == name)
    }
}

#[ext(name = TsObjectExt)]
pub(super) impl Object {
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
}
