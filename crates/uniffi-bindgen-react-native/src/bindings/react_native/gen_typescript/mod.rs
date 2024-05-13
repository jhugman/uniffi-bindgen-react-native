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
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uniffi_bindgen::interface::{Callable, FfiType, Type, UniffiTrait};
use uniffi_bindgen::ComponentInterface;

use super::ReactNativeConfig;
use crate::bindings::react_native::{ComponentInterfaceExt, FfiFunctionExt};

#[derive(Default)]
pub(crate) struct TsBindings {
    pub(crate) codegen: String,
    pub(crate) frontend: String,
}

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &ReactNativeConfig,
) -> Result<TsBindings> {
    let codegen = CodegenWrapper::new(ci, config)
        .render()
        .context("generating codegen bindings failed")?;
    let frontend = FrontendWrapper::new(ci, config)
        .render()
        .context("generating frontend javascript failed")?;

    Ok(TsBindings { codegen, frontend })
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper-ffi.ts")]
struct CodegenWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a ReactNativeConfig,
}

impl<'a> CodegenWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a ReactNativeConfig) -> Self {
        Self { ci, config }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper.ts")]
struct FrontendWrapper<'a> {
    ci: &'a ComponentInterface,
    config: &'a ReactNativeConfig,
    type_helper_code: String,
    type_imports: BTreeMap<String, BTreeSet<String>>,
}

impl<'a> FrontendWrapper<'a> {
    pub fn new(ci: &'a ComponentInterface, config: &'a ReactNativeConfig) -> Self {
        let type_renderer = TypeRenderer::new(config, ci);
        let type_helper_code = type_renderer.render().unwrap();
        let type_imports = type_renderer.imports.into_inner();
        Self {
            config,
            ci,
            type_helper_code,
            type_imports,
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
    #[allow(unused)]
    config: &'a ReactNativeConfig,
    ci: &'a ComponentInterface,
    // Track included modules for the `include_once()` macro
    include_once_names: RefCell<HashSet<String>>,
    // Track imports added with the `add_import()` macro
    imports: RefCell<BTreeMap<String, BTreeSet<String>>>,
}

impl<'a> TypeRenderer<'a> {
    fn new(config: &'a ReactNativeConfig, ci: &'a ComponentInterface) -> Self {
        Self {
            config,
            ci,
            include_once_names: RefCell::new(HashSet::new()),
            imports: RefCell::new(Default::default()),
        }
    }

    // The following methods are used by the `Types.ts` macros.

    // Helper for the including a template, but only once.
    //
    // The first time this is called with a name it will return true, indicating that we should
    // include the template.  Subsequent calls will return false.
    fn include_once_check(&self, name: &str) -> bool {
        self.include_once_names
            .borrow_mut()
            .insert(name.to_string())
    }

    // Helper to add an import statement
    //
    // Call this inside your template to cause an import statement to be added at the top of the
    // file.  Imports will be sorted and de-deuped.
    // ```
    // {{ self.add_import_from("foo", "bar")}}
    // ```
    // will cause output:
    // ```
    // import { foo } from "uniffi-bindgen-react-native/bar"
    // ```
    //
    // Returns an empty string so that it can be used inside an askama `{{ }}` block.
    fn add_import_from(&self, what: &str, from: &str) -> &str {
        let mut map = self.imports.borrow_mut();
        let set = map.entry(from.to_owned()).or_default();
        set.insert(what.to_owned());
        ""
    }
}
