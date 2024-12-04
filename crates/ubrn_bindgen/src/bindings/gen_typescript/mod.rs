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
    SwitchArgs,
};

#[derive(Default)]
pub(crate) struct TsBindings {
    pub(crate) codegen: String,
    pub(crate) frontend: String,
}

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
    switches: &SwitchArgs,
    type_map: &TypeMap,
) -> Result<TsBindings> {
    let codegen = CodegenWrapper::new(ci, config, module, switches)
        .render()
        .context("generating codegen bindings failed")?;
    let frontend = FrontendWrapper::new(ci, config, module, switches, type_map)
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
    #[allow(unused)]
    switches: &'a SwitchArgs,
}

impl<'a> CodegenWrapper<'a> {
    fn new(
        ci: &'a ComponentInterface,
        config: &'a Config,
        module: &'a ModuleMetadata,
        switches: &'a SwitchArgs,
    ) -> Self {
        Self {
            ci,
            config,
            module,
            switches,
        }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper.ts")]
struct FrontendWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    module: &'a ModuleMetadata,
    #[allow(unused)]
    switches: &'a SwitchArgs,
    type_helper_code: String,
    type_imports: BTreeMap<String, BTreeSet<Imported>>,
    exported_converters: BTreeSet<String>,
    imported_converters: BTreeMap<(String, String), BTreeSet<String>>,
}

impl<'a> FrontendWrapper<'a> {
    pub fn new(
        ci: &'a ComponentInterface,
        config: &'a Config,
        module: &'a ModuleMetadata,
        switches: &'a SwitchArgs,
        type_map: &'a TypeMap,
    ) -> Self {
        let type_renderer = TypeRenderer::new(ci, config, module, switches, type_map);
        let type_helper_code = type_renderer.render().unwrap();
        let type_imports = type_renderer.imports.into_inner();
        let exported_converters = type_renderer.exported_converters.into_inner();
        let imported_converters = type_renderer.imported_converters.into_inner();
        Self {
            module,
            config,
            ci,
            switches,
            type_helper_code,
            type_imports,
            exported_converters,
            imported_converters,
        }
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
    #[allow(unused)]
    switches: &'a SwitchArgs,

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
        switches: &'a SwitchArgs,
        type_map: &'a TypeMap,
    ) -> Self {
        Self {
            ci,
            config,
            module,
            switches,
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
        match external.as_type() {
            Type::External { namespace, .. } => {
                let type_ = self.as_type(external);
                let name = type_name(&type_, self).expect("External types should have type names");
                match &type_ {
                    Type::Enum { .. } => self.import_ext(&name, &namespace),
                    Type::CallbackInterface { .. }
                    | Type::Custom { .. }
                    | Type::Object { .. }
                    | Type::Record { .. } => self.import_ext_type(&name, &namespace),
                    _ => unreachable!(),
                };
                let ffi_converter_name = ffi_converter_name(&type_, self)
                    .expect("FfiConverter for External type will always exist");
                self.import_converter(ffi_converter_name, &namespace)
            }
            _ => unreachable!(),
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
            .filter(|t| !matches!(t, Type::External { .. }))
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
