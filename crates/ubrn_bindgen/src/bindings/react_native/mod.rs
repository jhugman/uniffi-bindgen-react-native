/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod gen_cpp;
mod gen_typescript;
mod uniffi_toml;

use std::{collections::HashMap, fs};

use anyhow::Result;
use camino::Utf8Path;
use extend::ext;
use heck::{ToLowerCamelCase, ToSnakeCase};
use topological_sort::TopologicalSort;
use ubrn_common::{fmt, run_cmd_quietly};
use uniffi_bindgen::{
    interface::{
        FfiArgument, FfiCallbackFunction, FfiDefinition, FfiField, FfiFunction, FfiStruct, FfiType,
        Function, Method, Object, UniffiTrait,
    },
    BindingGenerator, Component, ComponentInterface, GenerationSettings,
};
use uniffi_meta::Type;
use uniffi_toml::ReactNativeConfig;

use self::{gen_cpp::CppBindings, gen_typescript::TsBindings};

use super::{type_map::TypeMap, OutputArgs};
use crate::bindings::metadata::ModuleMetadata;

pub(crate) struct ReactNativeBindingGenerator {
    output: OutputArgs,
}

impl ReactNativeBindingGenerator {
    pub(crate) fn new(output: OutputArgs) -> Self {
        Self { output }
    }

    pub(crate) fn format_code(&self) -> Result<()> {
        format_ts(&self.output.ts_dir.canonicalize_utf8()?)?;
        format_cpp(&self.output.cpp_dir.canonicalize_utf8()?)?;
        Ok(())
    }
}

impl BindingGenerator for ReactNativeBindingGenerator {
    type Config = ReactNativeConfig;

    fn new_config(&self, root_toml: &toml::value::Value) -> Result<Self::Config> {
        Ok(match root_toml.get("bindings") {
            Some(v) => v.clone().try_into()?,
            None => Default::default(),
        })
    }

    fn update_component_configs(
        &self,
        _settings: &GenerationSettings,
        _components: &mut Vec<Component<Self::Config>>,
    ) -> Result<()> {
        // NOOP
        Ok(())
    }

    fn write_bindings(
        &self,
        settings: &GenerationSettings,
        components: &[Component<Self::Config>],
    ) -> Result<()> {
        let mut type_map = TypeMap::default();
        for component in components {
            type_map.insert_ci(&component.ci);
        }
        for component in components {
            let ci = &component.ci;
            let module: ModuleMetadata = component.into();
            let config = &component.config;
            let TsBindings { codegen, frontend } =
                gen_typescript::generate_bindings(ci, &config.typescript, &module, &type_map)?;

            let out_dir = &self.output.ts_dir.canonicalize_utf8()?;
            let codegen_path = out_dir.join(module.ts_ffi_filename());
            let frontend_path = out_dir.join(module.ts_filename());
            fs::write(codegen_path, codegen)?;
            fs::write(frontend_path, frontend)?;

            let out_dir = &self.output.cpp_dir.canonicalize_utf8()?;
            let CppBindings { hpp, cpp } = gen_cpp::generate_bindings(ci, &config.cpp, &module)?;
            let cpp_path = out_dir.join(module.cpp_filename());
            let hpp_path = out_dir.join(module.hpp_filename());
            fs::write(cpp_path, cpp)?;
            fs::write(hpp_path, hpp)?;
        }
        if settings.try_format_code {
            self.format_code()?;
        }
        Ok(())
    }
}

fn format_ts(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut prettier) = fmt::prettier(out_dir, false)? {
        run_cmd_quietly(&mut prettier)?
    } else {
        eprintln!("No prettier found. Install with `yarn add --dev prettier`");
    }
    Ok(())
}

fn format_cpp(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut clang_format) = fmt::clang_format(out_dir, false)? {
        run_cmd_quietly(&mut clang_format)?
    } else {
        eprintln!("Skipping formatting C++. Is clang-format installed?");
    }
    Ok(())
}

#[ext]
impl ComponentInterface {
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
            .chain(self.iter_ffi_functions_init_callback())
            .chain(self.iter_ffi_function_bless_pointer())
    }

    fn iter_ffi_functions_js_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        let has_async = self.has_async_fns();
        self.iter_ffi_function_definitions().filter(move |f| {
            let name = f.name();
            !name.contains("_rustbuffer_")
                && (has_async || !name.contains("_rust_future_"))
                && !name.contains("_callback_vtable_")
        })
    }

    fn iter_ffi_functions_cpp_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_rust()
    }

    fn iter_ffi_functions_init_callback(&self) -> impl Iterator<Item = FfiFunction> {
        self.callback_interface_definitions()
            .iter()
            .map(|cb| cb.ffi_init_callback().clone())
            .chain(self.object_definitions().iter().filter_map(|obj| {
                if obj.has_callback_interface() {
                    Some(obj.ffi_init_callback().clone())
                } else {
                    None
                }
            }))
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
            .filter(|s| !s.is_future() || s.name() == "ForeignFuture")
    }

    fn iter_ffi_definitions_exported_by_ts(&self) -> impl Iterator<Item = FfiDefinition> {
        self.ffi_definitions().filter(|d| d.is_exported())
    }

    fn cpp_namespace(&self) -> String {
        format!("uniffi::{}", self.namespace().to_snake_case())
    }

    fn cpp_namespace_includes(&self) -> String {
        "uniffi_jsi".to_string()
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
        for type_ in self.iter_types() {
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
        sorted.into_iter().chain(types.into_values())
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
impl Object {
    fn is_uniffi_trait(t: &UniffiTrait, nm: &str) -> bool {
        match t {
            UniffiTrait::Debug { .. } => nm == "Debug",
            UniffiTrait::Display { .. } => nm == "Display",
            UniffiTrait::Eq { .. } => nm == "Eq",
            UniffiTrait::Hash { .. } => nm == "Hash",
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
        let func: Method = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::RustArcPtr(String::from(""))),
            vec![FfiArgument::new("pointer", FfiType::UInt64)],
        );
        ffi
    }
}

#[ext]
impl FfiFunction {
    fn is_internal(&self) -> bool {
        let name = self.name();
        name.contains("ffi__") && name.contains("_internal_")
    }
}

#[ext]
impl FfiDefinition {
    fn is_exported(&self) -> bool {
        match self {
            Self::Function(_) => false,
            Self::CallbackFunction(cb) => cb.is_exported(),
            Self::Struct(s) => s.is_exported(),
        }
    }
}

#[ext]
impl FfiType {
    fn is_callable(&self) -> bool {
        matches!(self, Self::Callback(_))
    }

    fn is_void(&self) -> bool {
        matches!(self, Self::VoidPointer)
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
            | Self::RustArcPtr(_)
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
impl FfiArgument {
    fn is_return(&self) -> bool {
        self.name() == "uniffi_out_return"
    }
}

#[ext]
impl FfiCallbackFunction {
    fn cpp_namespace(&self, ci: &ComponentInterface) -> String {
        let ffi_type = FfiType::Callback(self.name().to_string());
        ffi_type.cpp_namespace(ci)
    }

    fn is_exported(&self) -> bool {
        let name = self.name();
        [
            "RustFutureContinuationCallback",
            "CallbackInterfaceFree",
            "ForeignFutureFree",
        ]
        .contains(&name)
            || !name.starts_with("CallbackInterface")
    }

    fn is_rust_calling_js(&self) -> bool {
        !self.is_future_callback() || self.name() == "RustFutureContinuationCallback"
    }

    fn is_free_callback(&self) -> bool {
        is_free(self.name())
    }

    fn is_future_callback(&self) -> bool {
        is_future(self.name())
    }

    fn arg_return_type(&self) -> Option<FfiType> {
        let arg = self
            .arguments()
            .into_iter()
            .find(|a| a.is_return() && !a.type_().is_void());
        arg.map(|a| a.type_())
    }

    fn is_blocking(&self) -> bool {
        self.name() != "RustFutureContinuationCallback"
    }
}

fn is_future(nm: &str) -> bool {
    nm.starts_with("ForeignFuture") || nm.starts_with("RustFuture")
}

fn is_free(nm: &str) -> bool {
    nm == "CallbackInterfaceFree" || nm == "ForeignFutureFree"
}

#[ext]
impl FfiStruct {
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
        self.is_vtable() || self.is_future()
    }

    fn is_future(&self) -> bool {
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
impl FfiField {
    fn is_free(&self) -> bool {
        matches!(self.type_(), FfiType::Callback(s) if is_free(&s))
    }
}
