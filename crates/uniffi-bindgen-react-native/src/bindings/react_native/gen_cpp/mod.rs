mod filters;

use super::ReactNativeConfig;
use anyhow::Result;
use askama::Template;
use std::borrow::Borrow;
use uniffi_bindgen::ComponentInterface;

#[derive(Debug, Default)]
pub(crate) struct CppBindings {
    pub(crate) hpp: String,
    pub(crate) cpp: String,
}

pub(crate) fn generate_bindings(
    ci: &ComponentInterface,
    config: &ReactNativeConfig,
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
    config: &'a ReactNativeConfig,
}

impl<'a> HppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a ReactNativeConfig) -> Self {
        Self { ci, config }
    }
}

#[derive(Template)]
#[template(syntax = "cpp", escape = "none", path = "wrapper.cpp")]
struct CppWrapper<'a> {
    ci: &'a ComponentInterface,
    #[allow(unused)]
    config: &'a ReactNativeConfig,
}

impl<'a> CppWrapper<'a> {
    fn new(ci: &'a ComponentInterface, config: &'a ReactNativeConfig) -> Self {
        Self { ci, config }
    }
}
