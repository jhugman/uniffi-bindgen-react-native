/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # The Scalar Marshalling Layer
//!
//! This module handles the mechanical conversion between JavaScript values and their C
//! representations for *scalar* types: integers, floats, and opaque handles. It does not
//! handle `RustBuffer`, callbacks, or struct (VTable) arguments — those are marshalled
//! directly in [`crate::register::call_ffi_function`].
//!
//! The marshalling operates in three phases that mirror the lifecycle of a single FFI call:
//!
//! ## Phase 1: JS to Boxed Rust (`js_to_boxed`)
//!
//! Each JS value is converted to the correctly-typed Rust scalar and heap-allocated via
//! `Box::new`. The heap allocation is necessary because libffi's [`Arg`] borrows a *pointer*
//! to the value, and the value must outlive the `cif.call` invocation. (This is a known
//! inefficiency — one heap allocation per scalar argument — but it keeps the marshalling
//! code straightforward and type-safe. A future optimization could use a bump allocator
//! or stack-allocated storage.)
//!
//! ## Phase 2: Boxed Rust to libffi Arg (`boxed_to_arg`)
//!
//! Each `Box<dyn Any>` is downcast to its concrete type and borrowed as a typed reference,
//! then wrapped in a libffi [`Arg`]. Because the `Arg` borrows from the box, the box must
//! outlive the arg vector. Both are held in `call_ffi_function`'s stack frame, so this
//! invariant is maintained naturally.
//!
//! ## Phase 3: Boxed Return to JS (`ret_to_js`)
//!
//! After `cif.call`, the return value arrives as a `Box<dyn Any>` (type-erased in
//! [`crate::register::call_with_ret_type`]). We downcast to the concrete type and create
//! the appropriate JS value. The JS type mappings are:
//!
//! | Rust type         | JS type   | Rationale                                                 |
//! |-------------------|-----------|-----------------------------------------------------------|
//! | `u8`..`u32`, `i8`..`i32` | `number` | `f64` has 53 bits of mantissa — sufficient for all 32-bit integers. |
//! | `u64`, `i64`      | `BigInt`  | 64-bit integers exceed `f64` precision.                   |
//! | `f32`, `f64`      | `number`  | JS `number` is IEEE-754 `f64`.                            |

use std::any::Any;

use libffi::middle::{arg, Arg};
use napi::bindgen_prelude::*;
use napi::{JsBigInt, JsNumber, JsUnknown, NapiRaw, NapiValue, Result};

use crate::ffi_type::FfiTypeDesc;

/// Downcast a `dyn Any` to a concrete type, returning a napi error on mismatch
/// instead of panicking.
fn downcast<'a, T: Any>(boxed: &'a dyn Any, desc: &FfiTypeDesc) -> Result<&'a T> {
    boxed.downcast_ref::<T>().ok_or_else(|| {
        napi::Error::from_reason(format!(
            "Type mismatch in FFI marshalling: expected {} for {desc:?}",
            std::any::type_name::<T>(),
        ))
    })
}

/// Convert a JS value to a heap-allocated Rust scalar matching `desc`.
///
/// The returned `Box<dyn Any>` holds the correctly-typed value (`u8`, `i32`, `u64`, etc.)
/// and is later downcast in [`boxed_to_arg`] to create a libffi `Arg`.
///
/// For types that fit in a JS `number` (integers up to 32 bits, floats), we go through
/// `JsNumber::get_double` and truncate. For 64-bit integers and handles, we expect a
/// `BigInt` on the JS side and use `JsBigInt::get_u64` / `get_i64`.
pub fn js_to_boxed(env: &Env, desc: &FfiTypeDesc, js_val: JsUnknown) -> Result<Box<dyn Any>> {
    match desc {
        FfiTypeDesc::UInt8 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as u8))
        }
        FfiTypeDesc::Int8 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as i8))
        }
        FfiTypeDesc::UInt16 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as u16))
        }
        FfiTypeDesc::Int16 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as i16))
        }
        FfiTypeDesc::UInt32 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as u32))
        }
        FfiTypeDesc::Int32 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as i32))
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            // SAFETY: `JsBigInt::from_raw` reconstructs a napi `JsBigInt` wrapper from
            // raw handles. `env.raw()` is the valid `napi_env` for the current call
            // context, and `js_val.raw()` is the `napi_value` we received as an argument
            // — both are live handles in the current scope. The `from_raw` call does not
            // take ownership; it merely wraps the handles for the typed API.
            let bigint = unsafe { JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _lossless) = bigint.get_u64()?;
            Ok(Box::new(v))
        }
        FfiTypeDesc::Int64 => {
            // SAFETY: Same as the UInt64/Handle arm above — `env` and `js_val` are
            // valid napi handles from the current call context.
            let bigint = unsafe { JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _lossless) = bigint.get_i64()?;
            Ok(Box::new(v))
        }
        FfiTypeDesc::Float32 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v as f32))
        }
        FfiTypeDesc::Float64 => {
            let n: JsNumber = js_val.try_into()?;
            let v: f64 = n.get_double()?;
            Ok(Box::new(v))
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported argument type for js_to_boxed: {desc:?}"
        ))),
    }
}

/// Create a libffi [`Arg`] by borrowing the concrete value inside a type-erased box.
///
/// The lifetime `'a` ties the `Arg` to the box, enforcing at compile time that the
/// heap-allocated value outlives the argument vector passed to `cif.call`. Each arm
/// downcasts to the same type that [`js_to_boxed`] originally boxed. A type mismatch
/// returns a descriptive error instead of panicking.
pub fn boxed_to_arg<'a>(desc: &FfiTypeDesc, boxed: &'a dyn Any) -> Result<Arg<'a>> {
    match desc {
        FfiTypeDesc::UInt8 => Ok(arg(downcast::<u8>(boxed, desc)?)),
        FfiTypeDesc::Int8 => Ok(arg(downcast::<i8>(boxed, desc)?)),
        FfiTypeDesc::UInt16 => Ok(arg(downcast::<u16>(boxed, desc)?)),
        FfiTypeDesc::Int16 => Ok(arg(downcast::<i16>(boxed, desc)?)),
        FfiTypeDesc::UInt32 => Ok(arg(downcast::<u32>(boxed, desc)?)),
        FfiTypeDesc::Int32 => Ok(arg(downcast::<i32>(boxed, desc)?)),
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => Ok(arg(downcast::<u64>(boxed, desc)?)),
        FfiTypeDesc::Int64 => Ok(arg(downcast::<i64>(boxed, desc)?)),
        FfiTypeDesc::Float32 => Ok(arg(downcast::<f32>(boxed, desc)?)),
        FfiTypeDesc::Float64 => Ok(arg(downcast::<f64>(boxed, desc)?)),
        FfiTypeDesc::RustCallStatus => {
            // The boxed value is a *mut RustCallStatusC.
            Ok(arg(downcast::<*mut u8>(boxed, desc)?))
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported argument type for boxed_to_arg: {desc:?}"
        ))),
    }
}

/// Convert a type-erased return value from `cif.call` into the corresponding JS value.
///
/// Called by `call_ffi_function` after the FFI call completes. The `boxed` value was
/// produced by [`call_with_ret_type`](crate::register::call_with_ret_type), which
/// monomorphized on the correct Rust type and boxed the result. We downcast and create
/// the JS representation: `number` for integers <= 32 bits and all floats, `BigInt`
/// for 64-bit integers, `undefined` for void.
pub fn ret_to_js(env: &Env, desc: &FfiTypeDesc, boxed: &dyn Any) -> Result<JsUnknown> {
    match desc {
        FfiTypeDesc::Void => Ok(env.get_undefined()?.into_unknown()),
        FfiTypeDesc::UInt8 => {
            let v = downcast::<u8>(boxed, desc)?;
            Ok(env.create_uint32(*v as u32)?.into_unknown())
        }
        FfiTypeDesc::Int8 => {
            let v = downcast::<i8>(boxed, desc)?;
            Ok(env.create_int32(*v as i32)?.into_unknown())
        }
        FfiTypeDesc::UInt16 => {
            let v = downcast::<u16>(boxed, desc)?;
            Ok(env.create_uint32(*v as u32)?.into_unknown())
        }
        FfiTypeDesc::Int16 => {
            let v = downcast::<i16>(boxed, desc)?;
            Ok(env.create_int32(*v as i32)?.into_unknown())
        }
        FfiTypeDesc::UInt32 => {
            let v = downcast::<u32>(boxed, desc)?;
            Ok(env.create_uint32(*v)?.into_unknown())
        }
        FfiTypeDesc::Int32 => {
            let v = downcast::<i32>(boxed, desc)?;
            Ok(env.create_int32(*v)?.into_unknown())
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            let v = downcast::<u64>(boxed, desc)?;
            Ok(env.create_bigint_from_u64(*v)?.into_unknown()?)
        }
        FfiTypeDesc::Int64 => {
            let v = downcast::<i64>(boxed, desc)?;
            Ok(env.create_bigint_from_i64(*v)?.into_unknown()?)
        }
        FfiTypeDesc::Float32 => {
            let v = downcast::<f32>(boxed, desc)?;
            Ok(env.create_double(*v as f64)?.into_unknown())
        }
        FfiTypeDesc::Float64 => {
            let v = downcast::<f64>(boxed, desc)?;
            Ok(env.create_double(*v)?.into_unknown())
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported return type for ret_to_js: {desc:?}"
        ))),
    }
}
