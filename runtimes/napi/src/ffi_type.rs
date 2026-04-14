/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! The abstract type language that drives the entire bridge.
//!
//! [`FfiTypeDesc`] is the Rosetta Stone of this system. Every JS-provided type
//! description — a tagged object like `{ tag: 'Int32' }` or
//! `{ tag: 'Reference', inner: { tag: 'Struct', name: 'MyStruct' } }` — is parsed
//! into one of these variants. From that single enum, every downstream decision
//! flows:
//!
//! - **CIF construction** ([`crate::cif::ffi_type_for`]): maps each variant to a
//!   concrete `libffi::middle::Type` so libffi knows the calling convention.
//! - **Marshalling** ([`crate::marshal`]): determines how JS values are converted
//!   to/from C-compatible byte representations.
//! - **Callback handling** ([`crate::callback`]): decides how to pack/unpack
//!   arguments when Rust invokes a JS-provided callback.
//!
//! The enum is **recursive**: `Reference` and `MutReference` wrap an inner
//! `FfiTypeDesc` in a `Box`, reflecting the pointer-to-T pattern that UniFFI uses
//! for pass-by-reference structs and mutable out-parameters.

use napi::{JsObject, Result};

/// The abstract type descriptor parsed from JS-provided definitions.
///
/// Mirrors UniFFI's `FfiType` enum. Each variant represents a type that can
/// appear in a foreign function signature. Parsed from tagged JS objects like
/// `{ tag: 'Int32' }` or `{ tag: 'Callback', name: 'cb_name' }`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum FfiTypeDesc {
    UInt8,
    Int8,
    UInt16,
    Int16,
    UInt32,
    Int32,
    UInt64,
    Int64,
    Float32,
    Float64,
    Handle,
    RustBuffer,
    ForeignBytes,
    RustCallStatus,
    Callback(String),
    Struct(String),
    Reference(Box<FfiTypeDesc>),
    MutReference(Box<FfiTypeDesc>),
    VoidPointer,
    Void,
}

impl FfiTypeDesc {
    /// Parse an `FfiTypeDesc` from a JS object with shape `{ tag: string, ...params }`.
    ///
    /// Recursively descends into `Reference` and `MutReference` wrappers,
    /// building the boxed inner type from the `inner` property of the JS object.
    pub fn from_js_object(obj: &JsObject) -> Result<Self> {
        let tag: String = obj.get_named_property::<String>("tag")?;
        match tag.as_str() {
            "UInt8" => Ok(Self::UInt8),
            "Int8" => Ok(Self::Int8),
            "UInt16" => Ok(Self::UInt16),
            "Int16" => Ok(Self::Int16),
            "UInt32" => Ok(Self::UInt32),
            "Int32" => Ok(Self::Int32),
            "UInt64" => Ok(Self::UInt64),
            "Int64" => Ok(Self::Int64),
            "Float32" => Ok(Self::Float32),
            "Float64" => Ok(Self::Float64),
            "Handle" => Ok(Self::Handle),
            "RustBuffer" => Ok(Self::RustBuffer),
            "ForeignBytes" => Ok(Self::ForeignBytes),
            "RustCallStatus" => Ok(Self::RustCallStatus),
            "VoidPointer" => Ok(Self::VoidPointer),
            "Void" => Ok(Self::Void),
            "Callback" => {
                let name: String = obj.get_named_property::<String>("name")?;
                Ok(Self::Callback(name))
            }
            "Struct" => {
                let name: String = obj.get_named_property::<String>("name")?;
                Ok(Self::Struct(name))
            }
            "Reference" => {
                let inner: JsObject = obj.get_named_property("inner")?;
                Ok(Self::Reference(Box::new(Self::from_js_object(&inner)?)))
            }
            "MutReference" => {
                let inner: JsObject = obj.get_named_property("inner")?;
                Ok(Self::MutReference(Box::new(Self::from_js_object(&inner)?)))
            }
            other => Err(napi::Error::from_reason(format!(
                "Unknown FfiType tag: {other}"
            ))),
        }
    }
}
