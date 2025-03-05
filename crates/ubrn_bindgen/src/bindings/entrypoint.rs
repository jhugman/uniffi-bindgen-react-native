/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use ubrn_common::CrateMetadata;

use crate::{AbiFlavor, ModuleMetadata, SwitchArgs};

use super::gen_cpp;
#[cfg(feature = "wasm")]
use super::gen_rust;

pub fn generate_entrypoint(
    switches: &SwitchArgs,
    crate_: &CrateMetadata,
    modules: &Vec<ModuleMetadata>,
) -> Result<String> {
    match &switches.flavor {
        AbiFlavor::Jsi => gen_cpp::generate_entrypoint(crate_, modules),
        #[cfg(feature = "wasm")]
        AbiFlavor::Wasm => gen_rust::generate_entrypoint(crate_, modules),
    }
}
