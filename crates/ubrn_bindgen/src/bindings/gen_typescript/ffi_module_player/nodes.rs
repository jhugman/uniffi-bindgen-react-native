/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use camino::Utf8PathBuf;

/// How the generated player should locate the cdylib at load time.
///
/// Maps 1:1 to the three resolveLibPath modes in `@ubjs/node`.
#[derive(Clone, Debug)]
pub enum LibResolution {
    /// Look for the conventional filename next to the binding.
    Colocated,
    /// Bake an absolute path into the generated code.
    Absolute(Utf8PathBuf),
    /// Resolve via `<base><triple>` platform npm packages.
    ///
    /// `base` is the literal prefix joined to the triple — callers are
    /// responsible for any separator (`-`, `/`, `_`); the runtime concatenates
    /// without inserting one. `triple_style` selects between cargo-style and
    /// node-style triple naming.
    Require {
        base: String,
        triple_style: TripleStyle,
    },
}

/// Which platform-triple naming convention the consuming npm packages use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TripleStyle {
    /// `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, … (cargo `--target`).
    #[default]
    Cargo,
    /// `darwin-arm64`, `linux-x64-gnu`, … (napi-rs convention).
    Node,
}

impl TripleStyle {
    /// String tag passed through to the runtime in generated TS.
    pub fn as_runtime_tag(self) -> &'static str {
        match self {
            TripleStyle::Cargo => "cargo",
            TripleStyle::Node => "node",
        }
    }
}

/// IR for the player-style `{namespace}-ffi.ts`.
///
/// Generates a `DEFINITIONS` object for the napi player's `register()` call,
/// plus a TypeScript type for the object it returns.
pub(crate) struct PlayerFfiModule {
    /// Whether to suppress `@ts-nocheck` for strict type checking.
    pub strict_type_checking: bool,
    /// The crate name, passed to `resolveLibPath` so error messages name it.
    pub crate_name: String,
    /// How the player should locate the library at runtime.
    pub lib_resolution: LibResolution,
    /// Rustbuffer management symbol names.
    pub symbols: PlayerSymbols,
    /// FFI function registrations for `register({ functions: { ... } })`.
    pub functions: Vec<PlayerFunctionDef>,
    /// Callback registrations for `register({ callbacks: { ... } })`.
    pub callbacks: Vec<PlayerCallbackDef>,
    /// Struct registrations for `register({ structs: { ... } })`.
    pub structs: Vec<PlayerStructDef>,
    /// Functions for the `NativeModuleInterface` TypeScript type.
    /// Uses the same IR as the JSI ffi module (for rendering the interface).
    pub typed_functions: Vec<super::super::ffi_module::FfiFunctionDecl>,
    /// Definitions (callbacks/structs) for TypeScript type exports.
    pub typed_definitions: Vec<super::super::ffi_module::FfiDefinitionDecl>,
}

pub(crate) struct PlayerSymbols {
    pub rustbuffer_alloc: String,
    pub rustbuffer_free: String,
    pub rustbuffer_from_bytes: String,
}

pub(crate) struct PlayerFunctionDef {
    /// The raw FFI symbol name (e.g. "uniffi_arithmetical_fn_func_add").
    pub name: String,
    /// Player FfiType expressions for arguments (e.g. "FfiType.UInt32").
    pub args: Vec<String>,
    /// Player FfiType expression for return (e.g. "FfiType.UInt32" or "FfiType.Void").
    pub ret: String,
    /// Whether this function has a trailing RustCallStatus argument.
    pub has_rust_call_status: bool,
}

pub(crate) struct PlayerCallbackDef {
    /// The callback name as registered (e.g. "CallbackInterfaceFree").
    pub name: String,
    /// Player FfiType expressions for arguments.
    pub args: Vec<String>,
    /// Player FfiType expression for return (e.g. "FfiType.UInt32" or "FfiType.Void").
    pub ret: String,
    /// Whether this callback has a trailing RustCallStatus argument.
    pub has_rust_call_status: bool,
    /// Whether this callback uses the out-return convention.
    pub out_return: bool,
}

pub(crate) struct PlayerStructDef {
    /// Struct name as registered (e.g. "VTable_Calculator").
    pub name: String,
    /// Fields with their names and FfiType expressions.
    pub fields: Vec<PlayerFieldDef>,
}

pub(crate) struct PlayerFieldDef {
    pub name: String,
    /// Player FfiType expression (e.g. "FfiType.Callback(\"calc_add\")").
    pub type_expr: String,
}
