/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod config;
mod filters;
mod util;

use std::borrow::Borrow;

use anyhow::Result;
use askama::Template;
use uniffi_bindgen::interface::{FfiDefinition, FfiType};
use uniffi_bindgen::ComponentInterface;

pub(crate) use self::{config::CppConfig as Config, util::format_directory};
use crate::bindings::{
    extensions::{
        ComponentInterfaceExt, FfiArgumentExt, FfiCallbackFunctionExt, FfiFieldExt, FfiStructExt,
        FfiTypeExt, ObjectExt,
    },
    metadata::ModuleMetadata,
};

pub(crate) fn generate_hpp(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
) -> Result<String> {
    Ok(HppWrapper::new(ci, config, module).render()?)
}

pub(crate) fn generate_cpp(
    ci: &ComponentInterface,
    config: &Config,
    module: &ModuleMetadata,
) -> Result<String> {
    Ok(CppWrapper::new(ci, config, module).render()?)
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
