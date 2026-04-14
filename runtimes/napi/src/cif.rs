/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Mapping from the abstract type system to libffi's concrete type system.
//!
//! This module is the bridge between [`FfiTypeDesc`](crate::ffi_type::FfiTypeDesc)
//! (our parsed, platform-independent type descriptions) and
//! [`libffi::middle::Type`] (the concrete ABI-level types that libffi uses to
//! build call-interface descriptors).
//!
//! The key design insight is that the mapping is surprisingly flat:
//!
//! - **Scalars** (`UInt8`..`Float64`, `Handle`) map one-to-one to libffi primitives.
//! - **`RustBuffer`** is a pass-by-value struct. Its layout is
//!   `{ u64, u64, pointer }` — a three-field `Type::structure`.
//! - **`Struct(name)`** is a pass-by-value struct whose layout is looked up from
//!   `struct_defs` at CIF construction time. Each field is recursively mapped via
//!   `ffi_type_for`, producing a `Type::structure` that matches the C layout.
//! - **Everything pointer-shaped** — `Reference`, `MutReference`, `Callback`,
//!   `VoidPointer`, and `RustCallStatus` (always passed as `&mut`) — collapses
//!   to a single `Type::pointer()` at the ABI level, regardless of what the
//!   pointer points to.
//! - **`ForeignBytes`** is intentionally unsupported: it is parseable from JS
//!   for completeness but never appears in actual UniFFI function signatures.

use std::collections::HashMap;

use libffi::middle::Type;

use crate::ffi_type::FfiTypeDesc;
use crate::structs::StructDef;

/// Maps an [`FfiTypeDesc`] to a [`libffi::middle::Type`] suitable for CIF construction.
///
/// This is the single point of truth for how our abstract types become ABI types.
///
/// The `struct_defs` parameter is required to resolve `Struct(name)` variants: when a
/// by-value struct appears in a signature, its field layout must be known to build the
/// correct `Type::structure`. Pass the full struct definitions map from the registration
/// pipeline so that any by-value struct fields are resolved correctly.
///
/// # Errors
///
/// - Returns an error on `ForeignBytes`, which has no CIF representation.
/// - Returns an error on `Struct(name)` when `name` is not found in `struct_defs`.
pub fn ffi_type_for(
    desc: &FfiTypeDesc,
    struct_defs: &HashMap<String, StructDef>,
) -> napi::Result<Type> {
    match desc {
        FfiTypeDesc::UInt8 => Ok(Type::u8()),
        FfiTypeDesc::Int8 => Ok(Type::i8()),
        FfiTypeDesc::UInt16 => Ok(Type::u16()),
        FfiTypeDesc::Int16 => Ok(Type::i16()),
        FfiTypeDesc::UInt32 => Ok(Type::u32()),
        FfiTypeDesc::Int32 => Ok(Type::i32()),
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => Ok(Type::u64()),
        FfiTypeDesc::Int64 => Ok(Type::i64()),
        FfiTypeDesc::Float32 => Ok(Type::f32()),
        FfiTypeDesc::Float64 => Ok(Type::f64()),
        FfiTypeDesc::VoidPointer
        | FfiTypeDesc::Reference(_)
        | FfiTypeDesc::MutReference(_)
        | FfiTypeDesc::Callback(_) => Ok(Type::pointer()),
        FfiTypeDesc::Void => Ok(Type::void()),
        FfiTypeDesc::RustCallStatus => Ok(Type::pointer()), // always passed as &mut
        FfiTypeDesc::RustBuffer => Ok(Type::structure(vec![
            Type::u64(),
            Type::u64(),
            Type::pointer(),
        ])),
        FfiTypeDesc::ForeignBytes => Err(napi::Error::from_reason(
            "ForeignBytes has no CIF representation; it is not used in UniFFI function signatures",
        )),
        FfiTypeDesc::Struct(name) => {
            let struct_def = struct_defs.get(name).ok_or_else(|| {
                napi::Error::from_reason(format!(
                    "Unknown struct type: '{name}'. \
                     Ensure it is defined in the structs section of register()."
                ))
            })?;
            let field_types = struct_def
                .fields
                .iter()
                .map(|f| ffi_type_for(&f.field_type, struct_defs))
                .collect::<napi::Result<Vec<_>>>()?;
            Ok(Type::structure(field_types))
        }
    }
}
