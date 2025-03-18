/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use clap::{Args, ValueEnum};

#[derive(Args, Clone, Debug)]
pub struct SwitchArgs {
    /// The flavor of bindings to produce.
    #[clap(long, default_value = "jsi")]
    pub flavor: AbiFlavor,
}

impl Default for SwitchArgs {
    fn default() -> Self {
        Self {
            flavor: AbiFlavor::Jsi,
        }
    }
}

impl SwitchArgs {
    pub fn flavor(&self) -> AbiFlavor {
        self.flavor.clone()
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum AbiFlavor {
    Jsi,
    #[cfg(feature = "wasm")]
    Wasm,
}

impl AbiFlavor {
    pub fn entrypoint(&self) -> &str {
        match self {
            Self::Jsi => "Entrypoint.cpp",
            #[cfg(feature = "wasm")]
            Self::Wasm => "src/lib.rs",
        }
    }
}
