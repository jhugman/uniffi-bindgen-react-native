/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! uniffi-runtime-core: engine-agnostic FFI mechanics for UniFFI player backends.
#![warn(clippy::undocumented_unsafe_blocks)]

mod call;
mod callback;
mod cif;
mod error;
mod ffi_type;
mod library;
mod lifecycle;
mod module;

pub mod ffi_c_types;
pub mod slot;
pub mod spec;

pub use call::{slot_size_align, ArgLayout, CallReturn, PreparedCall, SlotLayout};
pub use callback::{CallbackFnPtr, DispatchFn, IsJsThreadFn, OnJsThreadFn, VTableField};
pub use error::{Error, Result};
pub use ffi_type::FfiTypeDesc;
pub use library::LibraryHandle;
pub use module::{AbortCallbacksFn, Module, StructFieldLayout, StructLayout};
pub use spec::{CallbackDef, FunctionDef, ModuleSpec, RustBufferSymbols, StructDef, StructField};
