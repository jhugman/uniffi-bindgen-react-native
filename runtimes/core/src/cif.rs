/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Mapping from FfiTypeDesc to libffi::middle::Type.

use libffi::middle::Type;
use std::collections::HashMap;

use crate::spec::StructDef;
use crate::{Error, FfiTypeDesc, Result};

pub fn ffi_type_for(desc: &FfiTypeDesc, struct_defs: &HashMap<String, StructDef>) -> Result<Type> {
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
        // When RustCallStatus appears as a struct field, it is an inline value
        // with layout {i8, u64, u64, pointer} matching RustCallStatusC.
        // Function-level CIF builders push Type::pointer() directly for &mut args,
        // so this only affects struct field layout computation.
        FfiTypeDesc::RustCallStatus => Ok(Type::structure(vec![
            Type::i8(),
            Type::u64(),
            Type::u64(),
            Type::pointer(),
        ])),
        FfiTypeDesc::RustBuffer => Ok(Type::structure(vec![
            Type::u64(),
            Type::u64(),
            Type::pointer(),
        ])),
        FfiTypeDesc::ForeignBytes => Err(Error::UnsupportedType(
            "ForeignBytes has no CIF representation; it is not used in UniFFI function signatures"
                .into(),
        )),
        FfiTypeDesc::Struct(name) => {
            let def = struct_defs
                .get(name)
                .ok_or_else(|| Error::UnknownStruct(name.clone()))?;
            let field_types = def
                .fields
                .iter()
                .map(|f| ffi_type_for(&f.field_type, struct_defs))
                .collect::<Result<Vec<_>>>()?;
            Ok(Type::structure(field_types))
        }
    }
}
