/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

pub mod cli;
pub(crate) mod codegen;
pub(crate) mod commands;
pub(crate) mod config;
pub(crate) mod jsi;
#[cfg(feature = "wasm")]
pub(crate) mod wasm;
pub(crate) mod workspace;

pub mod test_utils;

pub use anyhow::{Error, Result};
pub use camino::Utf8PathBuf;

use config::ProjectConfig;
use ubrn_bindgen::AbiFlavor;

// Move the shared functionality here
pub(crate) trait AsConfig<T>
where
    T: TryFrom<ProjectConfig, Error = Error>,
{
    fn config_file(&self) -> Option<Utf8PathBuf>;
    fn get(&self) -> Option<T>;

    fn as_config(&self) -> Result<T> {
        if let Some(t) = self.get() {
            Ok(t)
        } else if let Some(f) = self.config_file() {
            let config: ProjectConfig = f.try_into()?;
            config.try_into()
        } else {
            anyhow::bail!("Could not find a suitable value")
        }
    }
}

impl TryFrom<Utf8PathBuf> for ProjectConfig {
    type Error = Error;

    fn try_from(value: Utf8PathBuf) -> Result<Self, Self::Error> {
        ubrn_common::read_from_file(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Platform {
    Android,
    Ios,
    #[cfg(feature = "wasm")]
    Wasm,
}

impl From<&Platform> for AbiFlavor {
    fn from(value: &Platform) -> Self {
        match value {
            #[cfg(feature = "wasm")]
            Platform::Wasm => AbiFlavor::Wasm,
            _ => AbiFlavor::Jsi,
        }
    }
}
