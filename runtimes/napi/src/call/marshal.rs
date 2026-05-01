/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Napi-specific marshalling between JS values and the core [`PreparedCall`] / [`CallReturn`].
//!
//! Two directions:
//!
//! - **JS -> Rust** ([`write_js_to_slot`]): convert a JS scalar into native-endian
//!   bytes and write them into the correct [`PreparedCall`] slot before an FFI call.
//!   Pointer / `RustBuffer` slot writes go through [`uniffi_runtime_core::slot`]
//!   directly from the call site.
//!
//! - **Rust -> JS** ([`read_return_to_js`]): take the typed [`CallReturn`]
//!   produced by `Module::call` and create the corresponding N-API JS value.
//!
//! Only scalar types are dispatched here.  Compound types (callbacks, VTable
//! structs, RustBuffers) are marshalled in [`crate::call`] because they need
//! TSFN setup or struct-building that goes beyond simple byte copies.
//!
//! [`PreparedCall`]: uniffi_runtime_core::PreparedCall
//! [`CallReturn`]: uniffi_runtime_core::CallReturn

use napi::bindgen_prelude::*;
use napi::{JsBigInt, JsNumber, JsUnknown, NapiRaw, NapiValue, Result};

use uniffi_runtime_core::slot;
use uniffi_runtime_core::CallReturn;
use uniffi_runtime_core::FfiTypeDesc;

// ---------------------------------------------------------------------------
// Slot-based marshalling for Module::call
// ---------------------------------------------------------------------------

/// Write a JS scalar value into a byte slot at the correct native type.
///
/// `RustBuffer`, `Callback`, and `Reference(Struct)` args are handled by the caller in
/// `register.rs` because they require special plumbing (TSFN setup, VTable
/// construction, etc.).
pub fn write_js_to_slot(
    env: &Env,
    desc: &FfiTypeDesc,
    js_val: JsUnknown,
    slot: &mut [u8],
) -> Result<()> {
    match desc {
        FfiTypeDesc::UInt8 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_u8(slot, n.get_double()? as u8);
        }
        FfiTypeDesc::Int8 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_i8(slot, n.get_double()? as i8);
        }
        FfiTypeDesc::UInt16 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_u16(slot, n.get_double()? as u16);
        }
        FfiTypeDesc::Int16 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_i16(slot, n.get_double()? as i16);
        }
        FfiTypeDesc::UInt32 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_u32(slot, n.get_double()? as u32);
        }
        FfiTypeDesc::Int32 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_i32(slot, n.get_double()? as i32);
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            // SAFETY: `JsBigInt::from_raw` reconstructs a napi `JsBigInt` wrapper from
            // raw handles. `env.raw()` and `js_val.raw()` are valid live handles in the
            // current call context. `from_raw` does not take ownership.
            let bigint = unsafe { JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _lossless) = bigint.get_u64()?;
            slot::write_u64(slot, v);
        }
        FfiTypeDesc::Int64 => {
            // SAFETY: Same rationale as the UInt64/Handle arm above.
            let bigint = unsafe { JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _lossless) = bigint.get_i64()?;
            slot::write_i64(slot, v);
        }
        FfiTypeDesc::Float32 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_f32(slot, n.get_double()? as f32);
        }
        FfiTypeDesc::Float64 => {
            let n: JsNumber = js_val.try_into()?;
            slot::write_f64(slot, n.get_double()?);
        }
        _ => {
            return Err(napi::Error::from_reason(format!(
                "Unsupported argument type for write_js_to_slot: {desc:?}"
            )));
        }
    }
    Ok(())
}

/// Convert a typed [`CallReturn`] into the corresponding N-API JS value.
///
/// The match is exhaustive over the scalar and pointer variants.
/// [`CallReturn::RustBuffer`] is handled by the caller in [`super::call_ffi_function`]
/// because it needs the library's free pointer for cleanup.
pub fn read_return_to_js(env: &Env, ret: &CallReturn) -> Result<JsUnknown> {
    match ret {
        CallReturn::Void => Ok(env.get_undefined()?.into_unknown()),
        CallReturn::U8(v) => Ok(env.create_uint32(*v as u32)?.into_unknown()),
        CallReturn::I8(v) => Ok(env.create_int32(*v as i32)?.into_unknown()),
        CallReturn::U16(v) => Ok(env.create_uint32(*v as u32)?.into_unknown()),
        CallReturn::I16(v) => Ok(env.create_int32(*v as i32)?.into_unknown()),
        CallReturn::U32(v) => Ok(env.create_uint32(*v)?.into_unknown()),
        CallReturn::I32(v) => Ok(env.create_int32(*v)?.into_unknown()),
        CallReturn::U64(v) => Ok(env.create_bigint_from_u64(*v)?.into_unknown()?),
        CallReturn::I64(v) => Ok(env.create_bigint_from_i64(*v)?.into_unknown()?),
        CallReturn::F32(v) => Ok(env.create_double(*v as f64)?.into_unknown()),
        CallReturn::F64(v) => Ok(env.create_double(*v)?.into_unknown()),
        CallReturn::Pointer(v) => Ok(env.create_bigint_from_u64(*v as u64)?.into_unknown()?),
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported return type for read_return_to_js: {ret:?}"
        ))),
    }
}
