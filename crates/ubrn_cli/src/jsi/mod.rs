/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
pub(crate) mod android;
mod bindings;
mod codegen;
pub(crate) mod crossplatform;
mod generate;
pub(crate) mod ios;

pub(crate) use bindings::bindings;
pub(crate) use codegen::get_files;
pub(crate) use generate::{CmdArg, TurboModuleArgs};
