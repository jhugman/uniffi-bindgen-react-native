/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
pub(crate) mod api_module;
mod config;
pub(crate) mod ffi_module;
pub(crate) mod ffi_module_player;
mod type_mapping;
mod util;

use anyhow::{Context, Result};
use askama::Template;

use self::api_module::{TsTypeDefinition, TsUniffiTrait};
use self::ffi_module::FfiDefinitionDecl;
pub(crate) use self::{config::TsConfig as Config, util::format_directory};

pub(crate) fn generate_lowlevel_code(ffi_module: ffi_module::TsFfiModule) -> Result<String> {
    LowlevelTsWrapper::new(ffi_module)
        .render()
        .context("generating lowlevel typescript from IR failed")
}

pub(crate) fn generate_player_lowlevel_code(
    player_module: ffi_module_player::PlayerFfiModule,
) -> Result<String> {
    PlayerLowlevelTsWrapper::new(player_module)
        .render()
        .context("generating player lowlevel typescript from IR failed")
}

pub(crate) fn generate_api_code_from_ir(api_module: api_module::TsApiModule) -> Result<String> {
    TsApiWrapperV2::new(api_module)
        .render()
        .context("generating wrapper typescript from IR failed")
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper.ts")]
struct TsApiWrapperV2 {
    module: api_module::TsApiModule,
}

impl TsApiWrapperV2 {
    fn new(module: api_module::TsApiModule) -> Self {
        Self { module }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper-ffi.ts")]
struct LowlevelTsWrapper {
    module: ffi_module::TsFfiModule,
}

impl LowlevelTsWrapper {
    fn new(module: ffi_module::TsFfiModule) -> Self {
        Self { module }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "wrapper-ffi-player.ts")]
struct PlayerLowlevelTsWrapper {
    module: ffi_module_player::PlayerFfiModule,
}

impl PlayerLowlevelTsWrapper {
    fn new(module: ffi_module_player::PlayerFfiModule) -> Self {
        Self { module }
    }
}
