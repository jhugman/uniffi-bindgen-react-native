/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod filters;
mod oracle;

mod callback_interface;
mod compounds;
mod custom;
mod enum_;
mod external;
mod miscellany;
mod object;
mod primitives;
mod record;

use anyhow::{Context, Result};
use askama::Template;
use filters::ffi_converter_name;
use heck::ToUpperCamelCase;
use oracle::CodeOracle;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uniffi_bindgen::interface::{Callable, FfiDefinition, FfiType, Type, UniffiTrait};
use uniffi_bindgen::ComponentInterface;
use uniffi_meta::{AsType, ExternalKind};

use crate::bindings::metadata::ModuleMetadata;
use crate::bindings::react_native::{
    ComponentInterfaceExt, FfiCallbackFunctionExt, FfiFunctionExt, FfiStructExt, ObjectExt,
};

#[derive(Default)]
pub(crate) struct TsBindings {
    pub(crate) codegen: String,
    pub(crate) frontend: String,
}

type Config = crate::bindings::react_native::uniffi_toml::TsConfig;

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
) -> Result<TsBindings> {
    let codegen = CodegenWrapper::new(ci, config, module)
        .render()
        .context("generating codegen bindings failed")?;
    let frontend = FrontendWrapper::new(ci, config, module)
        .render()
        .context("generating frontend javascript failed")?;

    Ok(TsBindings { codegen, frontend })
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper-ffi.ts")]
struct CodegenWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    module: &'a ModuleMetadata,
}

impl<'a> CodegenWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a Config, module: &'a ModuleMetadata) -> Self {
        Self { ci, config, module }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper.ts")]
struct FrontendWrapper<'a> {
    ci: &'a ComponentInterface,
    module: &'a ModuleMetadata,
    #[allow(unused)]
    config: &'a Config,
    type_helper_code: String,
    type_imports: BTreeMap<String, BTreeSet<Imported>>,
    exported_converters: BTreeSet<String>,
    imported_converters: BTreeMap<(String, String), BTreeSet<String>>,
}

impl<'a> FrontendWrapper<'a> {
    pub fn new(ci: &'a ComponentInterface, config: &'a Config, module: &'a ModuleMetadata) -> Self {
        let type_renderer = TypeRenderer::new(ci, config, module);
        let type_helper_code = type_renderer.render().unwrap();
        let type_imports = type_renderer.imports.into_inner();
        let exported_converters = type_renderer.exported_converters.into_inner();
        let imported_converters = type_renderer.imported_converters.into_inner();
        Self {
            module,
            config,
            ci,
            type_helper_code,
            type_imports,
            exported_converters,
            imported_converters,
        }
    }

    pub fn initialization_fns(&self) -> Vec<String> {
        self.ci
            .iter_types()
            .map(|t| CodeOracle.find(t))
            .filter_map(|ct| ct.initialization_fn())
            .collect()
    }
}

/// Renders Typescript helper code for all types
///
/// This template is a bit different than others in that it stores internal state from the render
/// process.  Make sure to only call `render()` once.
#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "Types.ts")]
pub struct TypeRenderer<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    #[allow(unused)]
    module: &'a ModuleMetadata,
    // Track imports added with the `add_import()` macro
    imports: RefCell<BTreeMap<String, BTreeSet<Imported>>>,

    exported_converters: RefCell<BTreeSet<String>>,

    // Track imports added with the `add_import()` macro
    imported_converters: RefCell<BTreeMap<(String, String), BTreeSet<String>>>,
}

impl<'a> TypeRenderer<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a Config, module: &'a ModuleMetadata) -> Self {
        Self {
            ci,
            config,
            module,
            imports: RefCell::new(Default::default()),
            exported_converters: RefCell::new(Default::default()),
            imported_converters: RefCell::new(Default::default()),
        }
    }

    // The following methods are used by the `Types.ts` macros.

    // Helper to add an import statement
    //
    // Call this inside your template to cause an import statement to be added at the top of the
    // file.  Imports will be sorted and de-deuped.
    // ```
    // {{ self.import_infra("foo", "bar")}}
    // ```
    // will cause output:
    // ```
    // import { foo } from "uniffi-bindgen-react-native/bar"
    // ```
    //
    // Returns an empty string so that it can be used inside an askama `{{ }}` block.
    fn import_infra(&self, what: &str, _from: &str) -> &str {
        self.add_import(
            Imported::JSType(what.to_owned()),
            "uniffi-bindgen-react-native",
        )
    }

    fn import_infra_type(&self, what: &str, _from: &str) -> &str {
        self.add_import(
            Imported::TSType(what.to_owned()),
            "uniffi-bindgen-react-native",
        )
    }

    fn import_custom(&self, what: &str, from: &str) -> &str {
        self.add_import(Imported::JSType(what.to_owned()), from)
    }

    fn import_ext(&self, what: &str, from: &str) -> &str {
        self.add_import(Imported::JSType(what.to_owned()), &format!("./{from}"))
    }

    fn import_ext_type(&self, what: &str, from: &str) -> &str {
        self.add_import(Imported::TSType(what.to_owned()), &format!("./{from}"))
    }

    fn add_import(&self, what: Imported, from: &str) -> &str {
        let mut map = self.imports.borrow_mut();
        let set = map.entry(from.to_owned()).or_default();
        set.insert(what);
        ""
    }

    fn import_external_type(&self, external: &impl AsType) -> &str {
        match external.as_type() {
            Type::External {
                namespace,
                name,
                kind,
                ..
            } => {
                match kind {
                    ExternalKind::DataClass => {
                        self.import_ext_type(&name, &namespace);
                    }
                    ExternalKind::Interface => {
                        self.import_ext(&name, &namespace);
                    }
                    ExternalKind::Trait => {
                        self.import_ext(&name, &namespace);
                    }
                }
                let converters = format!("uniffi{}Module", namespace.to_upper_camel_case());
                let src = format!("./{namespace}");
                let ffi_converter_name = ffi_converter_name(external)
                    .expect("FfiConverter for External type will always exist");
                self.import_converter(&ffi_converter_name, &src, &converters);
                ""
            }
            _ => unreachable!(),
        }
    }

    fn import_converter(&self, what: &str, src: &str, converters: &str) -> &str {
        let mut map = self.imported_converters.borrow_mut();
        let key = (src.to_owned(), converters.to_owned());
        let set = map.entry(key).or_default();
        set.insert(what.to_owned());
        ""
    }

    fn export_converter(&self, what: &str) -> &str {
        let mut set = self.exported_converters.borrow_mut();
        set.insert(what.to_owned());
        ""
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Imported {
    TSType(String),
    JSType(String),
}
