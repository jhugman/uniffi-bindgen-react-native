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
        FfiTypeExt,
    },
};
use anyhow::Result;
use askama::Template;
use std::borrow::Borrow;
use uniffi_bindgen::interface::FfiDefinition;
use uniffi_bindgen::ComponentInterface;

#[derive(Debug, Default)]
pub(crate) struct CppBindings {
    pub(crate) hpp: String,
    pub(crate) cpp: String,
}

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &ModuleMetadata,
) -> Result<CppBindings> {
    let hpp = HppWrapper::new(ci, config).render()?;
    let cpp = CppWrapper::new(ci, config).render()?;
    Ok(CppBindings { hpp, cpp })
}

#[derive(Template)]
#[template(syntax = "hpp", escape = "none", path = "wrapper.hpp")]
struct HppWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a ModuleMetadata,
}

impl<'a> HppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a ModuleMetadata) -> Self {
        Self { ci, config }
    }
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "wrapper.cpp")]
struct CppWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a ModuleMetadata,
}

impl<'a> CppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a ModuleMetadata) -> Self {
        Self { ci, config }
    }
}
