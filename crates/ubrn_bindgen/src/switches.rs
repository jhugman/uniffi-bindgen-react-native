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

    /// Generate call bodies that `await` the player, for a player that
    /// answers over a message port. Same as `asyncDelivery = true` in
    /// `uniffi.toml`, and overrides it.
    #[clap(long = "async")]
    pub async_delivery: bool,
}

impl Default for SwitchArgs {
    fn default() -> Self {
        Self {
            flavor: AbiFlavor::Jsi,
            async_delivery: false,
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
    Jsi2,
    Napi,
    #[cfg(feature = "wasm")]
    Wasm,
    #[cfg(feature = "wasm")]
    Wasm2,
}

impl AbiFlavor {
    pub fn entrypoint(&self) -> &str {
        match self {
            Self::Jsi => "Entrypoint.cpp",
            Self::Jsi2 => "", // No native entrypoint; the player shim is generic
            Self::Napi => "", // No native entrypoint needed
            #[cfg(feature = "wasm")]
            Self::Wasm => "src/lib.rs",
            #[cfg(feature = "wasm")]
            Self::Wasm2 => "",
        }
    }

    pub fn is_jsi(&self) -> bool {
        matches!(self, Self::Jsi)
    }

    /// Whether the native module is found on globalThis (JSI installs it there).
    pub fn supports_globalthis_native_module(&self) -> bool {
        matches!(self, Self::Jsi | Self::Jsi2)
    }

    /// Whether the runtime uses a player (dlopen + register) rather than
    /// compiled-in bindings.
    pub fn supports_player(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            matches!(self, Self::Napi | Self::Jsi2 | Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            matches!(self, Self::Napi | Self::Jsi2)
        }
    }

    /// Whether FFI function names on the native module use the `ubrn_` prefix.
    /// JSI and WASM both use this prefix; the player flavors (Napi, Jsi2) use
    /// raw symbol names.
    pub fn supports_ubrn_prefix(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            !matches!(self, Self::Napi | Self::Jsi2 | Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            !matches!(self, Self::Napi | Self::Jsi2)
        }
    }

    /// Whether the runtime uses a plain `{ code: 0 }` object for RustCallStatus.
    pub fn supports_plain_call_status(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            matches!(self, Self::Jsi | Self::Jsi2 | Self::Napi | Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            matches!(self, Self::Jsi | Self::Jsi2 | Self::Napi)
        }
    }

    pub fn supports_text_encoder(&self) -> bool {
        !matches!(self, Self::Jsi | Self::Jsi2)
    }

    pub fn supports_rust_backtrace(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            matches!(self, Self::Wasm | Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            false
        }
    }

    pub fn supports_finalization_registry(&self) -> bool {
        !matches!(self, Self::Jsi)
    }

    /// Whether this flavor initializes synchronously at module load.
    ///
    /// Sync flavors (JSI, Napi) call `initialize()` from the index.ts top
    /// level and treat `uniffiInitAsync` as a no-op for parity. Async
    /// flavors (Wasm) defer all initialization into `uniffiInitAsync`.
    pub fn supports_sync_initialization(&self) -> bool {
        matches!(self, Self::Jsi | Self::Jsi2 | Self::Napi)
    }

    /// Whether the bindgen emits an `index.ts` beside the per-module wrappers.
    ///
    /// It is the single import surface for a crate with several namespaces.
    /// jsi and wasm get an entrypoint from `ubrn_cli` instead — a turbo module
    /// and `index.web.ts` respectively — so a second index here would be
    /// redundant for them. jsi2's entrypoint only adds the player import and
    /// its guard in front of this index.
    pub fn supports_index_ts_at_generation(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            matches!(self, Self::Napi | Self::Jsi2 | Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            matches!(self, Self::Napi | Self::Jsi2)
        }
    }

    /// Whether the generated code can `await` the player. Only a player can be
    /// put behind a message port; compiled-in bindings cannot. Napi has a
    /// player but its index initializes at module load, where nothing can await.
    ///
    /// A future flavor joining must first fix the two branches that still call
    /// the player synchronously: `StringHelperTemplate.ts` without
    /// `TextEncoder`, and `ObjectTemplate.ts`'s `bless` without a
    /// `FinalizationRegistry`. Wasm2 takes neither.
    pub fn supports_async_delivery(&self) -> bool {
        self.supports_player() && !self.supports_sync_initialization()
    }

    /// The `--flavor` spelling, for error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Jsi => "jsi",
            Self::Jsi2 => "jsi2",
            Self::Napi => "napi",
            #[cfg(feature = "wasm")]
            Self::Wasm => "wasm",
            #[cfg(feature = "wasm")]
            Self::Wasm2 => "wasm2",
        }
    }

    /// Wasm2 specifically — the per-module wrapper stays environment-neutral,
    /// exporting `PLAYER_DEFINITIONS` / `setNativeModule` and leaving the
    /// opening of the `.wasm` to a generated entrypoint, so it bundles for
    /// node, browsers and React Native alike.
    pub fn is_wasm2(&self) -> bool {
        #[cfg(feature = "wasm")]
        {
            matches!(self, Self::Wasm2)
        }
        #[cfg(not(feature = "wasm"))]
        {
            false
        }
    }
}
