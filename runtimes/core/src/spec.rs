/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use crate::FfiTypeDesc;
use std::collections::HashMap;

/// A single field in a struct definition.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub field_type: FfiTypeDesc,
}

/// A parsed struct definition (list of fields).
#[derive(Debug, Clone)]
pub struct StructDef {
    pub fields: Vec<StructField>,
}

/// An alias for the common type used to pass struct definitions to CIF builders.
pub type StructDefs = HashMap<String, StructDef>;

/// Describes the signature of a Rust FFI function exported by a UniFFI library.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub args: Vec<FfiTypeDesc>,
    pub ret: FfiTypeDesc,
    pub has_rust_call_status: bool,
}

/// Describes the signature of a callback interface method that JS must implement.
#[derive(Debug, Clone)]
pub struct CallbackDef {
    pub args: Vec<FfiTypeDesc>,
    pub ret: FfiTypeDesc,
    pub has_rust_call_status: bool,
    pub out_return: bool,
}

/// The three RustBuffer lifecycle symbols that every UniFFI library exports.
#[derive(Debug, Clone)]
pub struct RustBufferSymbols {
    pub alloc: String,
    pub free: String,
    pub from_bytes: String,
}

/// Complete specification for a loaded UniFFI module.
#[derive(Debug, Clone)]
pub struct ModuleSpec {
    pub rustbuffer_symbols: RustBufferSymbols,
    pub functions: HashMap<String, FunctionDef>,
    pub callbacks: HashMap<String, CallbackDef>,
    pub structs: HashMap<String, StructDef>,
}
