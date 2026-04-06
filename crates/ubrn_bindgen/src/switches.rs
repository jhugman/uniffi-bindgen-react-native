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
    Napi,
    #[cfg(feature = "wasm")]
    Wasm,
}

impl AbiFlavor {
    pub fn entrypoint(&self) -> &str {
        match self {
            Self::Jsi => "Entrypoint.cpp",
            Self::Napi => "", // No native entrypoint needed
            #[cfg(feature = "wasm")]
            Self::Wasm => "src/lib.rs",
        }
    }

    pub fn is_jsi(&self) -> bool {
        matches!(self, Self::Jsi)
    }

    /// Whether the native module is found on globalThis (JSI installs it there).
    pub fn supports_globalthis_native_module(&self) -> bool {
        matches!(self, Self::Jsi)
    }

    /// Whether the runtime uses a player (dlopen + register) rather than
    /// compiled-in bindings.
    pub fn supports_player(&self) -> bool {
        matches!(self, Self::Napi)
    }

    /// Whether FFI function names on the native module use the `ubrn_` prefix.
    pub fn supports_ubrn_prefix(&self) -> bool {
        matches!(self, Self::Jsi)
    }

    /// Whether the runtime uses a plain `{ code: 0 }` object for RustCallStatus.
    pub fn supports_plain_call_status(&self) -> bool {
        matches!(self, Self::Jsi | Self::Napi)
    }

    pub fn supports_text_encoder(&self) -> bool {
        !matches!(self, Self::Jsi)
    }

    pub fn supports_rust_backtrace(&self) -> bool {
        false
    }

    pub fn supports_finalization_registry(&self) -> bool {
        !matches!(self, Self::Jsi)
    }
}
