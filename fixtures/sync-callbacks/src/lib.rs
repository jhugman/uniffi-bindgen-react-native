/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;

mod getters;
mod listeners;
mod trait_getters;

pub use getters::*;
pub use listeners::*;
pub use trait_getters::*;

uniffi::include_scaffolding!("sync_callbacks");
