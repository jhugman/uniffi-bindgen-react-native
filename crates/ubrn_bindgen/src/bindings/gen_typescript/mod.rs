/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod filters;
mod oracle;

mod callback_interface;
mod compounds;
mod config;
mod custom;
mod enum_;
mod miscellany;
mod object;
mod primitives;
mod record;
mod util;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::{Context, Result};
use askama::Template;
use heck::ToUpperCamelCase;
use uniffi_bindgen::interface::{AsType, Callable, FfiDefinition, FfiType, Type, UniffiTrait};
use uniffi_bindgen::ComponentInterface;

pub(crate) use self::{config::TsConfig as Config, util::format_directory};
use self::{
    filters::{ffi_converter_name, type_name},
    oracle::CodeOracle,
};
use crate::{
    bindings::{
        extensions::{
            ComponentInterfaceExt, FfiCallbackFunctionExt, FfiFunctionExt, FfiStructExt, ObjectExt,
        },
        metadata::ModuleMetadata,
        type_map::TypeMap,
    },
    switches::SwitchArgs,
    AbiFlavor,
};

pub(crate) fn generate_api_code(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
    switches: &SwitchArgs,
    type_map: &TypeMap,
) -> Result<String> {
    let flavor = TsFlavorParams::from(&switches.flavor);
    let types = TypeRenderer::new(ci, config, module, &flavor, type_map);
    TsApiWrapper::try_from(types)?
        .render()
        .context("generating frontend typescript failed")
}

pub(crate) fn generate_lowlevel_code(
    ci: &ComponentInterface,
    module: &ModuleMetadata,
) -> Result<String> {
    LowlevelTsWrapper::new(ci, module)
        .render()
        .context("generating lowlevel typescipt failed")
}

#[derive(Debug)]
struct TsFlavorParams<'a> {
    inner: &'a AbiFlavor,
}

impl<'a> From<&'a AbiFlavor> for TsFlavorParams<'a> {
    fn from(flavor: &'a AbiFlavor) -> Self {
        Self { inner: flavor }
    }
}

impl TsFlavorParams<'_> {
    pub(crate) fn is_jsi(&self) -> bool {
        matches!(self.inner, &AbiFlavor::Jsi)
    }

    pub(crate) fn supports_text_encoder(&self) -> bool {
        !matches!(self.inner, &AbiFlavor::Jsi)
    }

    pub(crate) fn supports_finalization_registry(&self) -> bool {
        !matches!(self.inner, &AbiFlavor::Jsi)
    }
    pub(crate) fn supports_rust_backtrace(&self) -> bool {
        false
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper-ffi.ts")]
struct LowlevelTsWrapper<'a> {
    ci: &'a ComponentInterface,
    module: &'a ModuleMetadata,
}

impl<'a> LowlevelTsWrapper<'a> {
    fn new(ci: &'a ComponentInterface, module: &'a ModuleMetadata) -> Self {
        Self { ci, module }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper.ts")]
struct TsApiWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    module: &'a ModuleMetadata,
    #[allow(unused)]
    flavor: &'a TsFlavorParams<'a>,
    type_helper_code: String,
    type_imports: BTreeMap<String, BTreeSet<Imported>>,
    exported_converters: BTreeSet<String>,
    imported_converters: BTreeMap<(String, String), BTreeSet<String>>,
}

impl<'a> TryFrom<TypeRenderer<'a>> for TsApiWrapper<'a> {
    type Error = anyhow::Error;

    fn try_from(types: TypeRenderer<'a>) -> Result<Self> {
        let type_helper_code = types.render()?;
        let type_imports = types.imports.into_inner();
        let exported_converters = types.exported_converters.into_inner();
        let imported_converters = types.imported_converters.into_inner();
        let module = types.module;
        let config = types.config;
        let ci = types.ci;
        let flavor = types.flavor;
        Ok(Self {
            module,
            config,
            ci,
            flavor,
            type_helper_code,
            type_imports,
            exported_converters,
            imported_converters,
        })
    }
}

/// Renders Typescript helper code for all types
///
/// This template is a bit different than others in that it stores internal state from the render
/// process.  Make sure to only call `render()` once.
#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "Types.ts")]
struct TypeRenderer<'a> {
    ci: &'a ComponentInterface,
    config: &'a Config,
    #[allow(unused)]
    module: &'a ModuleMetadata,
    #[allow(unused)]
    flavor: &'a TsFlavorParams<'a>,

    // Track imports added with the `add_import()` macro
    imports: RefCell<BTreeMap<String, BTreeSet<Imported>>>,

    exported_converters: RefCell<BTreeSet<String>>,

    // Track imports added with the `add_import()` macro
    imported_converters: RefCell<BTreeMap<(String, String), BTreeSet<String>>>,

    // The universe of types outside of this module. For tracking external types.
    type_map: &'a TypeMap,
}

impl<'a> TypeRenderer<'a> {
    fn new(
        ci: &'a ComponentInterface,
        config: &'a Config,
        module: &'a ModuleMetadata,
        flavor: &'a TsFlavorParams,
        type_map: &'a TypeMap,
    ) -> Self {
        Self {
            ci,
            config,
            module,
            flavor,
            imports: RefCell::new(Default::default()),
            exported_converters: RefCell::new(Default::default()),
            imported_converters: RefCell::new(Default::default()),
            type_map,
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
        let type_ = external.as_type();
        if self.ci.is_external(&type_) {
            let name = type_name(&type_, self).expect("External types should have type names");
            let module_path = type_
                .module_path()
                .expect("External types will have module paths");
            let namespace = self
                .ci
                .namespace_for_module_path(module_path)
                .expect("Module path should be mappable to namespace");
            match &type_ {
                Type::Enum { .. } => self.import_ext(&name, namespace),
                Type::CallbackInterface { .. }
                | Type::Custom { .. }
                | Type::Object { .. }
                | Type::Record { .. } => self.import_ext_type(&name, namespace),
                _ => unreachable!(),
            };
            let ffi_converter_name = ffi_converter_name(&type_, self)
                .expect("FfiConverter for External type will always exist");
            self.import_converter(ffi_converter_name, namespace)
        } else {
            ""
        }
    }

    fn import_converter(&self, ffi_converter_name: String, namespace: &str) -> &str {
        let converters = format!("uniffi{}Module", namespace.to_upper_camel_case());
        let src = format!("./{namespace}");

        let mut map = self.imported_converters.borrow_mut();
        let key = (src, converters);
        let set = map.entry(key).or_default();
        set.insert(ffi_converter_name);
        ""
    }

    fn export_converter(&self, what: &str) -> &str {
        let mut set = self.exported_converters.borrow_mut();
        set.insert(what.to_owned());
        ""
    }

    fn initialization_fns(&self) -> Vec<String> {
        self.ci
            .iter_sorted_types()
            .map(|t| CodeOracle.find(&t))
            .filter_map(|ct| ct.initialization_fn())
            .collect()
    }

    pub(crate) fn as_type(&self, as_type: &impl AsType) -> Type {
        self.type_map.as_type(as_type)
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Imported {
    TSType(String),
    JSType(String),
}
