/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # Function Pointer Wrapping: C function pointers as callable JS functions
//!
//! When a VTable callback receives a `Callback`-typed argument (e.g.,
//! `ForeignFutureCompleteRustBuffer`), the trampoline receives a raw C function
//! pointer. This module wraps that pointer as a callable JS function so that
//! JavaScript code can invoke it naturally.
//!
//! When the JS function is called, arguments are marshalled from JS to their C
//! representations—including struct-by-value via [`marshal_js_struct_to_bytes`]
//!—and the C function pointer is invoked through `Module::call_callback_ptr`, which
//! keeps all libffi usage inside core.
//!
//! ## Struct-by-value marshalling
//!
//! Struct layout (field sizes, alignments, offsets) comes from
//! `Module::struct_field_offsets`, which delegates to libffi inside core. Each
//! JS property is read by name, converted to its C byte representation, and
//! written at the correct offset in the buffer.
//!
//! ## RustBuffer inside structs
//!
//! When a struct field is `RustBuffer`, the JS side provides a `Uint8Array`.
//! We convert it to a `RustBufferC` (24 bytes: `{u64 capacity, u64 len, *mut u8 data}`)
//! via `rustbuffer_from_bytes`, then copy the struct bytes into the buffer at
//! the correct field offset.
//!
//! ## Nested structs
//!
//! `RustCallStatus` inside `ForeignFutureResult` is `{i8 code, RustBuffer error_buf}`.
//! When marshalling, we recursively handle nested `Struct` fields by calling
//! `marshal_js_struct_to_bytes` for each nested struct field.

use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;

use napi::{Env, JsFunction, JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::napi_utils;
use uniffi_runtime_core::ffi_c_types::RustBufferC;
use uniffi_runtime_core::slot;
use uniffi_runtime_core::{FfiTypeDesc, Module};

/// Marshal a JS object into a C struct byte buffer matching the platform's C struct layout.
///
/// Uses `Module::struct_field_offsets` to determine field sizes, alignments, and offsets.
/// Fields are read from the JS object by name and written at the correct byte offset.
///
/// # Arguments
///
/// * `env` - The napi environment (must be on the main thread)
/// * `js_obj` - The JS object whose properties map to struct fields
/// * `struct_name` - The name of the struct in the module spec
/// * `module` - The loaded module (provides struct defs and layout computation)
/// * `rb_from_bytes_ptr` - Pointer to `rustbuffer_from_bytes` for RustBuffer fields
pub fn marshal_js_struct_to_bytes(
    env: &Env,
    js_obj: &JsObject,
    struct_name: &str,
    module: &Arc<Module>,
    rb_from_bytes_ptr: *const c_void,
) -> Result<Vec<u8>> {
    let struct_def = module
        .spec_structs()
        .get(struct_name)
        .ok_or_else(|| napi::Error::from_reason(format!("unknown struct: {struct_name}")))?;
    let layout = module
        .struct_field_offsets(struct_name)
        .map_err(crate::core_err)?;

    let mut buf = vec![0u8; layout.total_size];
    for (field_def, field_layout) in struct_def.fields.iter().zip(layout.fields.iter()) {
        let js_val: JsUnknown = js_obj.get_named_property(&field_def.name)?;
        let slot = &mut buf[field_layout.offset..field_layout.offset + field_layout.size];
        marshal_field_to_bytes(
            env,
            &field_def.field_type,
            js_val,
            slot,
            module,
            rb_from_bytes_ptr,
        )?;
    }
    Ok(buf)
}

/// Marshal a single struct field's JS value into the given byte slot.
fn marshal_field_to_bytes(
    env: &Env,
    field_type: &FfiTypeDesc,
    js_val: JsUnknown,
    slot: &mut [u8],
    module: &Arc<Module>,
    rb_from_bytes_ptr: *const c_void,
) -> Result<()> {
    match field_type {
        FfiTypeDesc::UInt8 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_u8(slot, n.get_double()? as u8);
        }
        FfiTypeDesc::Int8 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_i8(slot, n.get_double()? as i8);
        }
        FfiTypeDesc::UInt16 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_u16(slot, n.get_double()? as u16);
        }
        FfiTypeDesc::Int16 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_i16(slot, n.get_double()? as i16);
        }
        FfiTypeDesc::UInt32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_u32(slot, n.get_double()? as u32);
        }
        FfiTypeDesc::Int32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_i32(slot, n.get_double()? as i32);
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _) = bigint.get_u64()?;
            slot::write_u64(slot, v);
        }
        FfiTypeDesc::Int64 => {
            let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _) = bigint.get_i64()?;
            slot::write_i64(slot, v);
        }
        FfiTypeDesc::Float32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_f32(slot, n.get_double()? as f32);
        }
        FfiTypeDesc::Float64 => {
            let n: napi::JsNumber = js_val.try_into()?;
            slot::write_f64(slot, n.get_double()?);
        }
        FfiTypeDesc::RustBuffer => {
            let rb = unsafe {
                napi_utils::js_uint8array_to_rust_buffer(env.raw(), js_val, rb_from_bytes_ptr)?
            };
            slot::write_rust_buffer(slot, rb);
        }
        FfiTypeDesc::Struct(name) => {
            // Recursively marshal nested struct
            let nested_obj: JsObject = js_val.try_into()?;
            let nested_bytes =
                marshal_js_struct_to_bytes(env, &nested_obj, name, module, rb_from_bytes_ptr)?;
            slot[..nested_bytes.len()].copy_from_slice(&nested_bytes);
        }
        FfiTypeDesc::RustCallStatus => {
            // RustCallStatus is an inline struct: {code: i8, error_buf: RustBuffer}
            // We use struct_field_offsets for the "RustCallStatus" synthetic struct.
            // However, RustCallStatus isn't in spec.structs—it has a fixed layout:
            // {i8 code, u64 capacity, u64 len, *mut u8 data}
            let status_obj: JsObject = js_val.try_into()?;
            let code: i32 = status_obj.get_named_property("code")?;
            slot[0] = code as i8 as u8;

            // Compute RustCallStatus field layout via core.
            // RustCallStatus maps to Type::structure([i8, u64, u64, pointer]).
            // We need the offsets of the capacity/len/data fields.
            let rcs_layout = compute_rust_call_status_layout()?;

            // errorBuf -> RustBuffer fields (capacity, len, data), default to zero
            let has_error_buf: bool = status_obj.has_named_property("errorBuf")?;
            if has_error_buf && code != 0 {
                let err_val: JsUnknown = status_obj.get_named_property("errorBuf")?;
                let rb = unsafe {
                    napi_utils::js_uint8array_to_rust_buffer(env.raw(), err_val, rb_from_bytes_ptr)?
                };
                let u64_size = std::mem::size_of::<u64>();
                let ptr_size = std::mem::size_of::<*mut u8>();
                slot::write_u64(
                    &mut slot[rcs_layout[1]..rcs_layout[1] + u64_size],
                    rb.capacity,
                );
                slot::write_u64(&mut slot[rcs_layout[2]..rcs_layout[2] + u64_size], rb.len);
                slot::write_pointer(
                    &mut slot[rcs_layout[3]..rcs_layout[3] + ptr_size],
                    rb.data as *const c_void,
                );
            }
            // else: zero-initialized slot is already correct for success status
        }
        FfiTypeDesc::Callback(cb_name) => {
            // Callback-typed struct field: create a trampoline and write the fn pointer.
            let js_fn = unsafe { napi::JsFunction::from_raw(env.raw(), js_val.raw())? };
            let user_data =
                crate::callback::create_callback_user_data(env, js_fn, cb_name, module)?;
            let fn_ptr = module
                .make_callback_trampoline(
                    cb_name,
                    crate::callback::on_js_thread,
                    crate::callback::dispatch_to_js_thread,
                    crate::callback::is_js_thread,
                    user_data,
                )
                .map_err(crate::core_err)?;
            slot::write_pointer(slot, fn_ptr);
        }
        other => {
            return Err(napi::Error::from_reason(format!(
                "Unsupported struct field type: {other:?}"
            )));
        }
    }
    Ok(())
}

/// Compute RustCallStatus field offsets using core's `ArgLayout::compute`.
///
/// RustCallStatus has a fixed layout: `{i8 code, u64 capacity, u64 len, *mut u8 data}`.
fn compute_rust_call_status_layout() -> Result<Vec<usize>> {
    let layout = uniffi_runtime_core::ArgLayout::compute(
        &[
            FfiTypeDesc::Int8,
            FfiTypeDesc::UInt64,
            FfiTypeDesc::UInt64,
            FfiTypeDesc::VoidPointer,
        ],
        false,
    )
    .map_err(crate::core_err)?;
    Ok(layout.arg_slots.iter().map(|s| s.offset).collect())
}

/// Marshal a single JS argument to its C byte representation.
///
/// This is used by `create_fn_pointer_wrapper` to convert each JS argument
/// into a `Vec<u8>` that `Module::call_callback_ptr` can pass to libffi.
fn marshal_arg_to_bytes(
    env: &Env,
    desc: &FfiTypeDesc,
    js_val: JsUnknown,
    module: &Arc<Module>,
) -> Result<Vec<u8>> {
    match desc {
        FfiTypeDesc::UInt8 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok(vec![n.get_double()? as u8])
        }
        FfiTypeDesc::Int8 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as i8).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::UInt16 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as u16).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::Int16 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as i16).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::UInt32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as u32).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::Int32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as i32).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _) = bigint.get_u64()?;
            Ok(v.to_ne_bytes().to_vec())
        }
        FfiTypeDesc::Int64 => {
            let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _) = bigint.get_i64()?;
            Ok(v.to_ne_bytes().to_vec())
        }
        FfiTypeDesc::Float32 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok((n.get_double()? as f32).to_ne_bytes().to_vec())
        }
        FfiTypeDesc::Float64 => {
            let n: napi::JsNumber = js_val.try_into()?;
            Ok(n.get_double()?.to_ne_bytes().to_vec())
        }
        FfiTypeDesc::RustBuffer => {
            let rb_from_bytes_ptr = module.rb_ops().from_bytes_ptr;
            let rb = unsafe {
                napi_utils::js_uint8array_to_rust_buffer(env.raw(), js_val, rb_from_bytes_ptr)?
            };
            // Transmute RustBufferC to its raw bytes.
            let rb_bytes: [u8; std::mem::size_of::<RustBufferC>()] =
                unsafe { std::mem::transmute(rb) };
            Ok(rb_bytes.to_vec())
        }
        FfiTypeDesc::Struct(name) => {
            let rb_from_bytes_ptr = module.rb_ops().from_bytes_ptr;
            let js_obj: JsObject = js_val.try_into()?;
            marshal_js_struct_to_bytes(env, &js_obj, name, module, rb_from_bytes_ptr)
        }
        FfiTypeDesc::VoidPointer
        | FfiTypeDesc::Callback(_)
        | FfiTypeDesc::Reference(_)
        | FfiTypeDesc::MutReference(_) => {
            // Pointer-sized: read as u64 bigint
            let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
            let (v, _) = bigint.get_u64()?;
            Ok(v.to_ne_bytes().to_vec())
        }
        other => Err(napi::Error::from_reason(format!(
            "Unsupported fn pointer arg type: {other:?}"
        ))),
    }
}

/// Wrap a C function pointer as a callable JS function.
///
/// The returned `JsFunction` captures the function pointer, callback name, and
/// module reference in a closure. When called from JS, it marshals each argument
/// to its C byte representation via `marshal_arg_to_bytes` and invokes the function
/// pointer via `Module::call_callback_ptr`, which handles all libffi interaction.
///
/// # Arguments
///
/// * `env` - The napi environment
/// * `fn_ptr` - The raw C function pointer to wrap
/// * `callback_name` - The name of the callback definition describing the function's signature
/// * `module` - The loaded module (provides callback defs, struct defs, and call_callback_ptr)
pub fn create_fn_pointer_wrapper(
    env: &Env,
    fn_ptr: *const c_void,
    callback_name: &str,
    module: &Arc<Module>,
) -> Result<JsFunction> {
    // Validate that the callback exists in the spec.
    let cb_def = module
        .spec_callbacks()
        .get(callback_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Unknown callback: {callback_name}")))?;

    let arg_types = Rc::new(cb_def.args.clone());
    // Store fn_ptr as usize so the closure is Send (required by create_function_from_closure).
    let fn_ptr_val = fn_ptr as usize;
    let module_ref = Arc::clone(module);
    let cb_name = callback_name.to_string();

    let js_func = env.create_function_from_closure("fn_pointer_wrapper", move |ctx| {
        let fn_ptr = fn_ptr_val as *const c_void;

        let mut arg_buffers: Vec<Vec<u8>> = Vec::with_capacity(arg_types.len());
        for (i, desc) in arg_types.iter().enumerate() {
            let js_val = ctx.get::<JsUnknown>(i)?;
            let buf = marshal_arg_to_bytes(ctx.env, desc, js_val, &module_ref)?;
            arg_buffers.push(buf);
        }

        module_ref
            .call_callback_ptr(&cb_name, fn_ptr, arg_buffers)
            .map_err(crate::core_err)?;
        ctx.env.get_undefined().map(|v| v.into_unknown())
    })?;

    Ok(js_func)
}
