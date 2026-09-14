/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{fmt::Display, str::FromStr};

use anyhow::{Error, Result};
use serde::Deserialize;

use crate::config::{ExtraArgs, ProjectConfig};

/// wasm2 generates no Rust shim crate and no project entrypoint, so this is
/// only how to build the user's crate and where to put the bindings.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct Wasm2Config {
    #[serde(default)]
    pub(crate) features: Option<Vec<String>>,

    #[serde(default)]
    pub(crate) default_features: Option<bool>,

    #[allow(unused)]
    #[serde(default = "Wasm2Config::default_targets")]
    pub(crate) targets: Vec<Target>,

    #[serde(default = "Wasm2Config::default_cargo_extras")]
    pub(crate) cargo_extras: ExtraArgs,

    #[serde(alias = "ts", alias = "typescript")]
    #[serde(deserialize_with = "ProjectConfig::opt_relative_path")]
    #[serde(default)]
    pub(crate) ts_bindings: Option<String>,

    #[serde(default = "Wasm2Config::default_rustflags")]
    pub(crate) rustflags: ExtraArgs,
}

impl Default for Wasm2Config {
    fn default() -> Self {
        ubrn_common::default()
    }
}

impl Wasm2Config {
    fn default_targets() -> Vec<Target> {
        vec![Target::Wasm32UnknownUnknown]
    }
    fn default_cargo_extras() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }
    /// Empty: an escape hatch for the user's own flags. The growable table
    /// export is added post-link by `ubrn_common::export_growable_table`.
    fn default_rustflags() -> ExtraArgs {
        let args: &[&str] = &[];
        args.into()
    }
}

#[derive(Debug, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    #[default]
    #[serde(rename = "wasm32-unknown-unknown")]
    Wasm32UnknownUnknown,
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "wasm32-unknown-unknown" => Self::Wasm32UnknownUnknown,
            _ => return Err(anyhow::anyhow!("Unsupported target: '{s}'")),
        })
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
        })
    }
}

impl Target {
    pub fn triple(&self) -> &'static str {
        match self {
            Self::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
        }
    }
}
