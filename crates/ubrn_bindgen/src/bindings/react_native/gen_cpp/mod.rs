/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod filters;

use crate::bindings::{
    metadata::ModuleMetadata,
    react_native::{
        ComponentInterfaceExt, FfiArgumentExt, FfiCallbackFunctionExt, FfiFieldExt, FfiStructExt,
        FfiTypeExt, ObjectExt,
    },
};
use anyhow::Result;
use rinja::Template;
use std::borrow::Borrow;
use uniffi_bindgen::interface::{FfiDefinition, FfiType};
use uniffi_bindgen::ComponentInterface;

type Config = crate::bindings::react_native::uniffi_toml::CppConfig;

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
