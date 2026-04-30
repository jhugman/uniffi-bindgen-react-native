/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// IR for the player-style `{namespace}-ffi.ts`.
///
/// Generates a `DEFINITIONS` object for the napi player's `register()` call,
/// plus a TypeScript type for the object it returns.
pub(crate) struct PlayerFfiModule {
    /// Whether to suppress `@ts-nocheck` for strict type checking.
    pub strict_type_checking: bool,
    /// Optional library path baked into the generated code.
    /// If None, the getter accepts a path argument at runtime.
    pub lib_path: Option<String>,
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
