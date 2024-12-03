/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod filters;
mod util;

use std::borrow::Borrow;

use anyhow::Result;
use askama::Template;
use uniffi_bindgen::interface::{FfiDefinition, FfiType};
use uniffi_bindgen::ComponentInterface;

pub(crate) use self::util::format_directory;
use crate::bindings::{
    extensions::{
        ComponentInterfaceExt, FfiArgumentExt, FfiCallbackFunctionExt, FfiFieldExt, FfiStructExt,
        FfiTypeExt, ObjectExt,
    },
    metadata::ModuleMetadata,
};

type Config = crate::bindings::uniffi_toml::CppConfig;

#[derive(Debug, Default)]
pub(crate) struct CppBindings {
    pub(crate) hpp: String,
    pub(crate) cpp: String,
}

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
) -> Result<CppBindings> {
    let hpp = HppWrapper::new(ci, config, module).render()?;
    let cpp = CppWrapper::new(ci, config, module).render()?;
    Ok(CppBindings { hpp, cpp })
}

#[derive(Template)]
#[template(syntax = "hpp", escape = "none", path = "wrapper.hpp")]
struct HppWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    module: &'a ModuleMetadata,
}

impl<'a> HppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a Config, module: &'a ModuleMetadata) -> Self {
        Self { ci, config, module }
    }
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "wrapper.cpp")]
struct CppWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a Config,
    module: &'a ModuleMetadata,
}

impl<'a> CppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a Config, module: &'a ModuleMetadata) -> Self {
        Self { ci, config, module }
    }
}

pub fn generate_entrypoint(modules: &Vec<ModuleMetadata>) -> Result<String> {
    let index = EntrypointCpp { modules };
    Ok(index.render()?)
}

#[derive(Template)]
#[template(path = "entrypoint.cpp", escape = "none")]
struct EntrypointCpp<'a> {
    modules: &'a Vec<ModuleMetadata>,
}
