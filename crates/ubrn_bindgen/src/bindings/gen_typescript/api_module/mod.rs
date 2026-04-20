/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Pre-computed IR for the `wrapper.ts` template.
//!
//! Built from `uniffi_bindgen::pipeline::general::Namespace`.
//! Templates branch on pre-computed booleans and strings but still
//! assemble output structure (method bodies, class shapes).

mod builders;
mod docstring;
mod nodes;
mod type_helpers;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use heck::ToUpperCamelCase;
use uniffi_bindgen::pipeline::general;

use crate::{
    bindings::gen_typescript::{ffi_module, Config},
    switches::AbiFlavor,
};

use self::builders::*;
use self::docstring::format_docstring;
use self::nodes::*;

pub(crate) use self::nodes::{
    InitializationIR, TsCallable, TsCallbackInterface, TsCustomType, TsEnum, TsExternalType,
    TsFunction, TsObject, TsRecord, TsSimpleWrapper, TsTypeDefinition, TsUniffiTrait,
};

pub(crate) struct TsApiModule {
    pub module_name: String,
    pub namespace_docstring: Option<String>,
    pub strict_type_checking: bool,
    pub flavor: AbiFlavor,
    pub is_debug: bool,
    pub is_verbose: bool,
    pub supports_rust_backtrace: bool,
    pub console_import: Option<String>,
    pub file_imports: Vec<TsFileImport>,
    pub converter_imports: Vec<TsConverterImport>,
    pub exported_converters: BTreeSet<String>,
    pub type_definitions: Vec<TsTypeDefinition>,
    pub functions: Vec<TsFunction>,
    pub initialization: InitializationIR,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ImportedItem {
    Type(String),
    Value(String),
}

pub(crate) struct ImportAccumulator {
    imports: BTreeMap<String, BTreeSet<ImportedItem>>,
    exported_converters: BTreeSet<String>,
    imported_converters: BTreeMap<(String, String), BTreeSet<String>>,
}

impl ImportAccumulator {
    pub fn new() -> Self {
        Self {
            imports: BTreeMap::new(),
            exported_converters: BTreeSet::new(),
            imported_converters: BTreeMap::new(),
        }
    }

    pub fn add_infra_value(&mut self, name: &str) {
        self.imports
            .entry("uniffi-bindgen-react-native".into())
            .or_default()
            .insert(ImportedItem::Value(name.into()));
    }

    pub fn add_infra_type(&mut self, name: &str) {
        self.imports
            .entry("uniffi-bindgen-react-native".into())
            .or_default()
            .insert(ImportedItem::Type(name.into()));
    }

    pub fn add_ext_value(&mut self, name: &str, namespace: &str) {
        self.imports
            .entry(format!("./{namespace}"))
            .or_default()
            .insert(ImportedItem::Value(name.into()));
    }

    pub fn add_ext_type(&mut self, name: &str, namespace: &str) {
        self.imports
            .entry(format!("./{namespace}"))
            .or_default()
            .insert(ImportedItem::Type(name.into()));
    }

    pub fn add_custom_value(&mut self, name: &str, from: &str) {
        self.imports
            .entry(from.into())
            .or_default()
            .insert(ImportedItem::Value(name.into()));
    }

    pub fn add_exported_converter(&mut self, name: &str) {
        self.exported_converters.insert(name.into());
    }

    pub fn add_imported_converter(&mut self, converter: &str, namespace: &str) {
        let src = format!("./{namespace}");
        let converters = format!("uniffi{}Module", namespace.to_upper_camel_case());
        self.imported_converters
            .entry((src, converters))
            .or_default()
            .insert(converter.into());
    }

    pub fn merge(&mut self, other: Self) {
        for (k, v) in other.imports {
            self.imports.entry(k).or_default().extend(v);
        }
        self.exported_converters.extend(other.exported_converters);
        self.imported_converters.extend(other.imported_converters);
    }

    pub fn collect_base_imports(&mut self) {
        self.add_infra_value("RustBuffer");
        self.add_infra_value("UniffiInternalError");
        self.add_infra_value("UniffiRustCaller");
    }

    pub fn collect_primitive(&mut self, cfg: &Config, ty: &general::Type) {
        if matches!(ty, general::Type::String) {
            return;
        }
        if let Some(name) = type_helpers::ffi_converter_name_for_type(cfg, ty) {
            self.add_infra_value(&name);
        }
        match ty {
            general::Type::Timestamp => self.add_infra_type("UniffiTimestamp"),
            general::Type::Duration => self.add_infra_type("UniffiDuration"),
            _ => {}
        }
    }

    pub fn collect_type_definition(&mut self, td: &TsTypeDefinition) {
        match td {
            TsTypeDefinition::SimpleWrapper(w) => self.collect_simple_wrapper(w),
            TsTypeDefinition::StringHelper(_) => self.collect_string_helper(),
            TsTypeDefinition::Custom(c) => self.collect_custom(c),
            TsTypeDefinition::External(e) => self.collect_external(e),
            TsTypeDefinition::FlatEnum(e)
            | TsTypeDefinition::FlatError(e)
            | TsTypeDefinition::TaggedEnum(e) => self.collect_enum(e),
            TsTypeDefinition::Record(r) => self.collect_record(r),
            TsTypeDefinition::Object(o) => self.collect_object(o),
            TsTypeDefinition::CallbackInterface(cbi) => self.collect_callback_interface(cbi),
        }
    }

    fn collect_simple_wrapper(&mut self, w: &TsSimpleWrapper) {
        self.add_infra_value(&w.infra_class);
    }

    fn collect_string_helper(&mut self) {
        self.add_infra_type("UniffiByteArray");
        self.add_infra_value("uniffiCreateFfiConverterString");
    }

    fn collect_custom(&mut self, c: &TsCustomType) {
        self.add_infra_type("FfiConverter");
        self.add_infra_value("uniffiTypeNameSymbol");
        if let Some(cfg) = &c.custom_config {
            for (name, from) in &cfg.imports {
                self.add_custom_value(name, from);
            }
        }
        self.add_exported_converter(&c.ffi_converter_name);
    }

    fn collect_external(&mut self, e: &TsExternalType) {
        if e.is_enum_type {
            self.add_ext_value(&e.type_name, &e.module_path);
        } else {
            self.add_ext_type(&e.type_name, &e.module_path);
        }
        self.add_imported_converter(&e.converter_name, &e.module_path);
    }

    fn collect_enum(&mut self, e: &TsEnum) {
        self.add_infra_value("AbstractFfiConverterByteArray");
        self.add_infra_value("FfiConverterInt32");
        self.add_infra_value("UniffiInternalError");

        if e.is_error {
            self.add_infra_value("UniffiError");
            self.add_infra_value("uniffiTypeNameSymbol");
            self.add_infra_value("variantOrdinalSymbol");
        } else {
            self.add_infra_value("UniffiEnum");
        }

        self.add_exported_converter(&e.ffi_converter_name);

        if e.has_callables() {
            self.add_infra_value("uniffiTypeNameSymbol");
        }
        self.collect_uniffi_traits(&e.uniffi_traits);
        self.collect_callables(&e.constructors);
        self.collect_callables(&e.methods);
    }

    fn collect_record(&mut self, r: &TsRecord) {
        self.add_infra_value("uniffiCreateRecord");
        self.add_infra_value("AbstractFfiConverterByteArray");
        self.add_exported_converter(&r.ffi_converter_name);

        if r.has_callables() {
            self.add_infra_value("uniffiTypeNameSymbol");
        }
        self.collect_uniffi_traits(&r.uniffi_traits);
        self.collect_callables(&r.constructors);
        self.collect_callables(&r.methods);
    }

    fn collect_uniffi_traits(&mut self, traits: &Vec<TsUniffiTrait>) {
        for ut in traits {
            match ut {
                TsUniffiTrait::Display { method }
                | TsUniffiTrait::Debug { method }
                | TsUniffiTrait::Hash { method }
                | TsUniffiTrait::Ord { cmp: method } => self.collect_callable(method),
                TsUniffiTrait::Eq { eq, ne } => {
                    self.collect_callable(eq);
                    self.collect_callable(ne);
                }
            }
        }
    }

    fn collect_callables(&mut self, callables: &Vec<TsCallable>) {
        for callable in callables {
            self.collect_callable(callable);
        }
    }

    fn collect_object(&mut self, o: &TsObject) {
        self.add_infra_value("UniffiAbstractObject");
        self.add_infra_type("UniffiHandle");
        self.add_infra_value("FfiConverterObject");
        self.add_infra_type("UniffiObjectFactory");
        self.add_infra_type("FfiConverter");
        self.add_infra_type("UniffiGcObject");
        self.add_infra_value("destructorGuardSymbol");
        self.add_infra_value("pointerLiteralSymbol");
        self.add_infra_value("uniffiTypeNameSymbol");

        if o.is_error {
            self.add_infra_value("UniffiThrownObject");
        }

        if o.has_callback_interface {
            self.add_infra_value("FfiConverterObjectWithCallbacks");
        }

        if o.is_error {
            self.add_infra_value("FfiConverterObjectAsError");
            self.add_exported_converter(&o.ffi_error_converter_name);
        }

        self.add_exported_converter(&o.ffi_converter_name);

        if let Some(ref ctor) = o.primary_constructor {
            self.collect_callable(ctor);
        }
        for ctor in &o.alternate_constructors {
            self.collect_callable(ctor);
        }
        for method in &o.methods {
            self.collect_callable(method);
        }
        self.collect_uniffi_traits(&o.uniffi_traits);

        if let Some(ref vtable) = o.vtable {
            self.collect_vtable_imports(vtable);
        }
    }

    fn collect_callback_interface(&mut self, cbi: &TsCallbackInterface) {
        self.add_infra_value("FfiConverterCallback");
        self.collect_vtable_imports(&cbi.vtable);
        for method in &cbi.methods {
            self.collect_callable(method);
        }
    }

    fn collect_vtable_imports(&mut self, vtable: &TsVtable) {
        self.add_infra_type("UniffiHandle");
        self.add_infra_type("UniffiReferenceHolder");
        self.add_infra_type("UniffiByteArray");
        self.add_infra_value("UniffiResult");
        self.add_infra_type("UniffiRustCallStatus");

        for field in &vtable.fields {
            if let Some(ref method) = field.method {
                if method.is_async() {
                    if method.is_throwing() {
                        self.add_infra_value("uniffiTraitInterfaceCallAsyncWithError");
                    } else {
                        self.add_infra_value("uniffiTraitInterfaceCallAsync");
                    }
                } else if method.is_throwing() {
                    self.add_infra_value("uniffiTraitInterfaceCallWithError");
                } else {
                    self.add_infra_value("uniffiTraitInterfaceCall");
                }
            }
        }
    }

    fn collect_callable(&mut self, callable: &TsCallable) {
        if callable.is_async() {
            self.add_infra_value("uniffiRustCallAsync");
        }
    }

    pub fn collect_verbose_imports(&mut self, has_async: bool) {
        self.add_infra_type("UniffiHandle");
        self.add_infra_type("UniffiRustCallStatus");
        if has_async {
            self.add_infra_type("UniffiRustFutureContinuationCallback");
        }
    }
}

impl TsApiModule {
    fn build_type_definitions(
        cfg: &Config,
        namespace: &general::Namespace,
        flavor: &AbiFlavor,
    ) -> Vec<TsTypeDefinition> {
        let mut defs = Vec::new();

        let ffi_fn_types: HashMap<String, &general::FfiFunctionType> = namespace
            .ffi_definitions
            .iter()
            .filter_map(|def| match def {
                general::FfiDefinition::FunctionType(ft) => Some((ft.name.0.clone(), ft)),
                _ => None,
            })
            .collect();

        let mut string_helper_emitted = false;

        // Defer wrapper FfiConverters (Optional/Sequence/Map) until after base types
        // to avoid temporal-dead-zone errors where a wrapper references a converter
        // that hasn't been initialised yet.
        let mut deferred_wrappers: Vec<TsTypeDefinition> = Vec::new();

        for td in &namespace.type_definitions {
            match td {
                general::TypeDefinition::Simple(node) => {
                    if matches!(node.ty, general::Type::String) && !string_helper_emitted {
                        string_helper_emitted = true;
                        defs.push(TsTypeDefinition::StringHelper(build_string_helper(flavor)));
                    }
                }
                general::TypeDefinition::Optional(opt) => {
                    deferred_wrappers
                        .push(TsTypeDefinition::SimpleWrapper(build_optional(cfg, opt)));
                }
                general::TypeDefinition::Sequence(seq) => {
                    deferred_wrappers
                        .push(TsTypeDefinition::SimpleWrapper(build_sequence(cfg, seq)));
                }
                general::TypeDefinition::Map(map) => {
                    deferred_wrappers.push(TsTypeDefinition::SimpleWrapper(build_map(cfg, map)));
                }
                general::TypeDefinition::Custom(custom) => {
                    let td = TsTypeDefinition::Custom(build_custom_type(cfg, custom));
                    if matches!(
                        custom.builtin.ty,
                        general::Type::Map { .. }
                            | general::Type::Sequence { .. }
                            | general::Type::Optional { .. }
                    ) {
                        deferred_wrappers.push(td);
                    } else {
                        defs.push(td);
                    }
                }
                general::TypeDefinition::External(ext) => {
                    defs.push(TsTypeDefinition::External(build_external_type(cfg, ext)));
                }
                general::TypeDefinition::Enum(e) => {
                    let ts_enum = build_enum(cfg, e, flavor);
                    if ts_enum.is_flat && ts_enum.is_error {
                        defs.push(TsTypeDefinition::FlatError(ts_enum));
                    } else if ts_enum.is_flat {
                        defs.push(TsTypeDefinition::FlatEnum(ts_enum));
                    } else {
                        defs.push(TsTypeDefinition::TaggedEnum(ts_enum));
                    }
                }
                general::TypeDefinition::Record(r) => {
                    defs.push(TsTypeDefinition::Record(build_record(cfg, r, flavor)));
                }
                general::TypeDefinition::Interface(i) => {
                    defs.push(TsTypeDefinition::Object(Box::new(build_object(
                        cfg,
                        i,
                        flavor,
                        &ffi_fn_types,
                        cfg.strict_object_types,
                    ))));
                }
                general::TypeDefinition::CallbackInterface(cbi) => {
                    defs.push(TsTypeDefinition::CallbackInterface(
                        build_callback_interface(cfg, cbi, &ffi_fn_types, flavor),
                    ));
                }
            }
        }

        defs.append(&mut deferred_wrappers);

        defs
    }

    fn collect_all_imports(&self) -> ImportAccumulator {
        let mut acc = ImportAccumulator::new();
        acc.collect_base_imports();

        for td in &self.type_definitions {
            acc.collect_type_definition(td);
        }

        for func in &self.functions {
            acc.collect_callable(func);
        }

        if self.is_verbose {
            let has_async = self.functions.iter().any(|f| f.is_async())
                || self.type_definitions.iter().any(|td| match td {
                    TsTypeDefinition::Object(o) => {
                        o.methods.iter().any(|m| m.is_async())
                            || o.primary_constructor.as_ref().is_some_and(|c| c.is_async())
                            || o.alternate_constructors.iter().any(|c| c.is_async())
                    }
                    TsTypeDefinition::CallbackInterface(cbi) => cbi.has_async_methods,
                    _ => false,
                });
            if has_async {
                acc.collect_verbose_imports(true);
            } else {
                let has_any_callables = !self.functions.is_empty()
                    || self.type_definitions.iter().any(|td| {
                        matches!(
                            td,
                            TsTypeDefinition::Object(_) | TsTypeDefinition::CallbackInterface(_)
                        )
                    });
                if has_any_callables {
                    acc.collect_verbose_imports(false);
                }
            }
        }

        acc
    }

    pub(crate) fn from_general(
        cfg: &Config,
        namespace: &general::Namespace,
        flavor: AbiFlavor,
        ffi_exported_definitions: Vec<ffi_module::FfiExportedName>,
    ) -> Self {
        let module_name = namespace.name.clone();
        let namespace_docstring = namespace.docstring.as_deref().map(format_docstring);
        let supports_rust_backtrace = flavor.supports_rust_backtrace();
        let type_definitions = Self::build_type_definitions(cfg, namespace, &flavor);
        let functions = build_functions(cfg, namespace, &flavor);
        let initialization = build_initialization(namespace, &flavor);

        let mut primitive_imports = ImportAccumulator::new();
        for td in &namespace.type_definitions {
            if let general::TypeDefinition::Simple(node) = td {
                primitive_imports.collect_primitive(cfg, &node.ty);
            }
        }

        let mut module = Self {
            module_name,
            namespace_docstring,
            strict_type_checking: cfg.strict_type_checking,
            flavor,
            is_debug: cfg.is_debug(),
            is_verbose: cfg.is_verbose(),
            supports_rust_backtrace,
            console_import: cfg.console_import.clone(),
            file_imports: Vec::new(),
            converter_imports: Vec::new(),
            exported_converters: BTreeSet::new(),
            type_definitions,
            functions,
            initialization,
        };

        let mut acc = module.collect_all_imports();
        acc.merge(primitive_imports);

        // Build file imports: FFI types first, then cross-module imports
        let mut file_imports = Vec::new();

        if !ffi_exported_definitions.is_empty() {
            file_imports.push(TsFileImport {
                path: format!("./{}-ffi", module.module_name),
                types: ffi_exported_definitions
                    .iter()
                    .map(|def| def.name().to_string())
                    .collect(),
                values: Vec::new(),
            });
        }

        for (file, things) in acc.imports {
            let mut types = Vec::new();
            let mut values = Vec::new();
            for thing in things {
                match thing {
                    ImportedItem::Type(t) => types.push(t),
                    ImportedItem::Value(v) => values.push(v),
                }
            }
            file_imports.push(TsFileImport {
                path: file,
                types,
                values,
            });
        }

        module.file_imports = file_imports;

        // Build converter imports
        module.converter_imports = acc
            .imported_converters
            .into_iter()
            .map(|((path, default_name), converters)| TsConverterImport {
                path,
                default_name,
                converters: converters.into_iter().collect(),
            })
            .collect();

        module.exported_converters = acc.exported_converters;

        module
    }
}
