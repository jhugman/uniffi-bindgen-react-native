/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod bindings;
mod cli;
mod react_native;
mod switches;
#[cfg(feature = "wasm")]
mod wasm;

pub use self::{
    bindings::{generate_entrypoint, metadata::ModuleMetadata},
    cli::{BindingsArgs, OutputArgs, SourceArgs},
    switches::{AbiFlavor, SwitchArgs},
};
