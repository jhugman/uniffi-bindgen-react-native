/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod gen_cpp;
mod gen_typescript;

use std::{collections::HashMap, fs};

use anyhow::Result;
use camino::Utf8Path;
use extend::ext;
use serde::Deserialize;
use topological_sort::TopologicalSort;
use ubrn_common::{fmt, run_cmd_quietly};
use uniffi_bindgen::{
    interface::{FfiArgument, FfiFunction, FfiType, Function},
    BindingGenerator, BindingsConfig, ComponentInterface,
};
use uniffi_meta::Type;

use self::{gen_cpp::CppBindings, gen_typescript::TsBindings};

use super::OutputArgs;
use crate::bindings::metadata::ModuleMetadata;

#[derive(Deserialize)]
pub(crate) struct ReactNativeConfig {
    #[serde(skip)]
    namespace: String,
}

impl BindingsConfig for ReactNativeConfig {
    fn update_from_ci(&mut self, ci: &ComponentInterface) {
        self.namespace = ci.namespace().to_string();
    }

    fn update_from_cdylib_name(&mut self, _cdylib_name: &str) {
        // NOOP
    }

    fn update_from_dependency_configs(&mut self, _config_map: HashMap<&str, &Self>) {
        // NOOP
    }
}

impl From<&ReactNativeConfig> for ModuleMetadata {
    fn from(value: &ReactNativeConfig) -> Self {
        ModuleMetadata::new(&value.namespace)
    }
}

pub(crate) struct ReactNativeBindingGenerator {
    output: OutputArgs,
}

impl ReactNativeBindingGenerator {
    pub(crate) fn new(output: OutputArgs) -> Self {
        Self { output }
    }

    pub(crate) fn format_code(&self) -> Result<()> {
        if !self.output.no_format {
            format_ts(&self.output.ts_dir.canonicalize_utf8()?)?;
            format_cpp(&self.output.cpp_dir.canonicalize_utf8()?)?;
        }
        Ok(())
    }
}

impl BindingGenerator for ReactNativeBindingGenerator {
    type Config = ReactNativeConfig;

    fn write_bindings(
        &self,
        ci: &ComponentInterface,
        _config: &Self::Config,
        // We get the output directories from the OutputArgs
        _out_dir: &Utf8Path,
        // We will format the code all at once instead of here
        _try_format_code: bool,
    ) -> Result<()> {
        let module = ModuleMetadata::new(ci.namespace());
        let TsBindings { codegen, frontend } = gen_typescript::generate_bindings(ci, &module)?;

        let out_dir = &self.output.ts_dir.canonicalize_utf8()?;
        let codegen_path = out_dir.join(module.ts_ffi_filename());
        let frontend_path = out_dir.join(module.ts_filename());
        fs::write(codegen_path, codegen)?;
        fs::write(frontend_path, frontend)?;

        let out_dir = &self.output.cpp_dir.canonicalize_utf8()?;
        let CppBindings { hpp, cpp } = gen_cpp::generate_bindings(ci, &module)?;
        let cpp_path = out_dir.join(module.cpp_filename());
        let hpp_path = out_dir.join(module.hpp_filename());
        fs::write(cpp_path, cpp)?;
        fs::write(hpp_path, hpp)?;

        Ok(())
    }

    fn check_library_path(
        &self,
        _library_path: &Utf8Path,
        _cdylib_name: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

fn format_ts(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut prettier) = fmt::prettier(out_dir)? {
        run_cmd_quietly(&mut prettier)?
    } else {
        eprintln!("No prettier found. Install with `yarn add --dev prettier`");
    }
    Ok(())
}

fn format_cpp(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut clang_format) = fmt::clang_format(out_dir)? {
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
    }

    fn iter_ffi_functions_js_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        let has_async = self.has_async_fns();
        let has_callbacks = true;
        self.iter_ffi_function_definitions().filter(move |f| {
            let name = f.name();
            !name.contains("_rustbuffer_")
                && (has_async || !name.contains("_rust_future_"))
                && (has_callbacks || !name.contains("_callback_vtable_"))
        })
    }

    fn iter_ffi_functions_cpp_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_rust()
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
            sorted.extend(graph.pop_all().iter().filter_map(|name| types.remove(name)));
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
impl FfiFunction {
    fn is_internal(&self) -> bool {
        self.name().contains("ffi__")
    }
}

#[ext]
impl FfiType {
    fn requires_argument_cleanup(&self) -> bool {
        // If this returns true, there is expected a Bridging<{{ self|ffi_type_name() }}>.argument_cleanup(v).
        match self {
            Self::RustBuffer(_) => true, // includes/RustBuffer.h
            _ => false,
        }
    }
}
