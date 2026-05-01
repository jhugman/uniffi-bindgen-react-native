/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! The abstract type language that drives the entire bridge.

#[derive(Debug, Clone)]
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
