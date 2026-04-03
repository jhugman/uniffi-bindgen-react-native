/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Top-level IR node for `{namespace}-ffi.ts`.
///
/// All names and types are fully resolved strings; the template
/// iterates and interpolates without logic.
pub(crate) struct TsFfiModule {
    pub module_name: String,
    pub strict_type_checking: bool,
    pub is_jsi: bool,
    pub functions: Vec<FfiFunctionDecl>,
    /// Interleaved in the order from `general::Namespace::ffi_definitions`.
    pub definitions: Vec<FfiDefinitionDecl>,
    pub has_continuation_callback: bool,
    pub has_foreign_future: bool,
}

pub(crate) enum FfiExportedName {
    Callback(String),
    Struct(String),
}

impl FfiExportedName {
    pub fn name(&self) -> &str {
        match self {
            Self::Callback(n) | Self::Struct(n) => n,
        }
    }
}

impl TsFfiModule {
    pub(crate) fn exported_names(&self) -> Vec<FfiExportedName> {
        self.definitions
            .iter()
            .filter_map(|def| match def {
                FfiDefinitionDecl::Callback(cb) if cb.exported => {
                    Some(FfiExportedName::Callback(cb.name.clone()))
                }
                FfiDefinitionDecl::Struct(s) if s.exported => {
                    Some(FfiExportedName::Struct(s.name.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

pub(crate) struct FfiFunctionDecl {
    /// Includes the `ubrn_` prefix.
    pub name: String,
    pub arguments: Vec<FfiArgDecl>,
    pub return_type: Option<String>,
}

pub(crate) struct FfiArgDecl {
    /// camelCase, except `"uniffi_out_err"` which keeps its uniffi convention name.
    pub name: String,
    pub type_name: String,
}

pub(crate) enum FfiDefinitionDecl {
    Callback(FfiCallbackDecl),
    Struct(FfiStructDecl),
}

pub(crate) struct FfiCallbackDecl {
    pub exported: bool,
    pub name: String,
    /// Excludes output parameters (`uniffi_out_return`, `uniffi_out_dropped_callback`).
    pub arguments: Vec<FfiArgDecl>,
    /// `"void"` for non-blocking, `"UniffiResult<void>"` for blocking
    /// with no return value, or the FFI type name otherwise.
    pub return_type: String,
}

pub(crate) struct FfiStructDecl {
    pub exported: bool,
    pub name: String,
    pub fields: Vec<FfiFieldDecl>,
}

pub(crate) struct FfiFieldDecl {
    pub name: String,
    pub type_name: String,
}
