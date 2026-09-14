/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod bindings;
mod codegen;
mod commands;
mod config;
mod generate;

pub(crate) use bindings::bindings;
pub(crate) use codegen::get_files;
#[allow(unused_imports)]
pub(crate) use commands::Wasm2BuildArgs;
#[allow(unused_imports)]
pub(crate) use config::Wasm2Config;
pub(crate) use generate::CmdArg;
