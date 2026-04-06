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
//! representations — including struct-by-value via [`marshal_js_struct_to_bytes`]
//! — and the C function pointer is invoked through libffi.
//!
//! ## Struct-by-value marshalling
//!
//! The key challenge is marshalling JS objects into C struct byte buffers that
//! match the exact layout libffi expects. We use `Type::structure()` to build
//! the libffi type, then query the raw `ffi_type` to determine field sizes,
//! alignments, and offsets. Each JS property is read by name, converted to its
//! C byte representation, and written at the correct offset in the buffer.
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

use std::any::Any;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use libffi::middle::{arg, Arg, Cif, CodePtr, Type};
use libffi::raw::ffi_abi_FFI_DEFAULT_ABI;
use napi::{Env, JsFunction, JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::callback::CallbackDef;
use crate::cif::ffi_type_for;
use crate::ffi_c_types::{RustBufferC, RustBufferOps};
use crate::ffi_type::FfiTypeDesc;
use crate::marshal;
use crate::napi_utils;
use crate::structs::StructDef;

/// Force libffi to compute size and alignment for a struct type (and any nested structs).
///
/// `Type::structure()` creates `ffi_type` with `size=0, alignment=0`. These fields
/// are only populated when `ffi_prep_cif` walks the type tree. This function creates
/// a minimal dummy CIF with the struct as a parameter type, which triggers the
/// in-place initialization of the struct's `ffi_type` and all nested struct types.
fn prep_struct_type(struct_type: &Type) -> Result<()> {
    unsafe {
        let raw = struct_type.as_raw_ptr();
        // Build a minimal arg types array: [struct_type_ptr, null]
        let mut arg_types: [*mut libffi::low::ffi_type; 2] = [raw, std::ptr::null_mut()];
        let mut cif: libffi::low::ffi_cif = std::mem::zeroed();
        let status = libffi::raw::ffi_prep_cif(
            &mut cif as *mut _,
            ffi_abi_FFI_DEFAULT_ABI,
            1,
            &raw mut libffi::low::types::void as *mut _,
            arg_types.as_mut_ptr(),
        );
        if status != libffi::raw::ffi_status_FFI_OK {
            return Err(napi::Error::from_reason(format!(
                "ffi_prep_cif failed for struct type (status {status})"
            )));
        }
    }
    Ok(())
}

/// Compute field offsets for a libffi struct type.
///
/// After `Type::structure()` is called, libffi populates the internal `ffi_type`
/// with size, alignment, and element information. This function walks the elements
/// array and computes the byte offset of each field, respecting alignment padding.
fn struct_field_offsets(struct_type: &Type) -> Vec<usize> {
    let raw = struct_type.as_raw_ptr();
    let mut offsets = Vec::new();
    let mut offset = 0usize;
    unsafe {
        let elements = (*raw).elements;
        let mut i = 0;
        while !(*elements.add(i)).is_null() {
            let field_type = *elements.add(i);
            let field_align = (*field_type).alignment as usize;
            offset = (offset + field_align - 1) & !(field_align - 1);
            offsets.push(offset);
            offset += (*field_type).size;
            i += 1;
        }
    }
    offsets
}

/// Get the total size of a libffi type.
fn ffi_type_size(t: &Type) -> usize {
    unsafe { (*t.as_raw_ptr()).size }
}

/// Marshal a JS object into a C struct byte buffer matching the libffi struct layout.
///
/// Uses libffi's `Type::structure()` to determine field sizes, alignments, and offsets.
/// Fields are read from the JS object by name and written at the correct byte offset.
///
/// # Arguments
///
/// * `env` - The napi environment (must be on the main thread)
/// * `js_obj` - The JS object whose properties map to struct fields
/// * `struct_def` - The struct definition describing field names and types
/// * `struct_defs` - All struct definitions (needed for recursive/nested structs)
/// * `rb_from_bytes_ptr` - Pointer to `rustbuffer_from_bytes` for RustBuffer fields
pub fn marshal_js_struct_to_bytes(
    env: &Env,
    js_obj: &JsObject,
    struct_def: &StructDef,
    struct_defs: &HashMap<String, StructDef>,
    rb_from_bytes_ptr: *const c_void,
) -> Result<Vec<u8>> {
    // 1. Build the libffi Type::structure() to get size and field offsets
    let field_ffi_types: Vec<Type> = struct_def
        .fields
        .iter()
        .map(|f| ffi_type_for(&f.field_type, struct_defs))
        .collect::<napi::Result<Vec<_>>>()?;
    let struct_type = Type::structure(field_ffi_types);

    // Force libffi to compute size/alignment for this struct (and any nested structs).
    // Type::structure() initializes size=0 and alignment=0; these only get populated
    // when ffi_prep_cif walks the type tree. We call prep_cif directly on the raw
    // pointer so it mutates the struct_type in place.
    prep_struct_type(&struct_type)?;

    // 2. Allocate a Vec<u8> of the struct's total size
    let total_size = ffi_type_size(&struct_type);
    let mut buffer = vec![0u8; total_size];

    // 3. Get field offsets
    let offsets = struct_field_offsets(&struct_type);

    // 4. For each field, read the JS property, convert to C bytes, write at offset
    for (i, field) in struct_def.fields.iter().enumerate() {
        let offset = offsets[i];
        let js_val: JsUnknown = js_obj.get_named_property(&field.name)?;

        match &field.field_type {
            FfiTypeDesc::UInt8 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as u8;
                buffer[offset] = v;
            }
            FfiTypeDesc::Int8 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as i8;
                buffer[offset..offset + 1].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::UInt16 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as u16;
                buffer[offset..offset + 2].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::Int16 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as i16;
                buffer[offset..offset + 2].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::UInt32 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as u32;
                buffer[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::Int32 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as i32;
                buffer[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
                let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
                let (v, _) = bigint.get_u64()?;
                buffer[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::Int64 => {
                let bigint = unsafe { napi::JsBigInt::from_raw(env.raw(), js_val.raw())? };
                let (v, _) = bigint.get_i64()?;
                buffer[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::Float32 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()? as f32;
                buffer[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::Float64 => {
                let n: napi::JsNumber = js_val.try_into()?;
                let v = n.get_double()?;
                buffer[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
            }
            FfiTypeDesc::RustBuffer => {
                // Convert JS Uint8Array -> RustBufferC via rustbuffer_from_bytes
                let rb = unsafe {
                    napi_utils::js_uint8array_to_rust_buffer(env.raw(), js_val, rb_from_bytes_ptr)?
                };
                // RustBufferC is { capacity: u64, len: u64, data: *mut u8 } = 24 bytes
                let rb_bytes: [u8; std::mem::size_of::<RustBufferC>()] =
                    unsafe { std::mem::transmute(rb) };
                buffer[offset..offset + rb_bytes.len()].copy_from_slice(&rb_bytes);
            }
            FfiTypeDesc::Struct(name) => {
                // Recursively marshal nested struct
                let nested_obj: JsObject = js_val.try_into()?;
                let nested_def = struct_defs.get(name).ok_or_else(|| {
                    napi::Error::from_reason(format!("Unknown nested struct type: '{name}'"))
                })?;
                let nested_bytes = marshal_js_struct_to_bytes(
                    env,
                    &nested_obj,
                    nested_def,
                    struct_defs,
                    rb_from_bytes_ptr,
                )?;
                buffer[offset..offset + nested_bytes.len()].copy_from_slice(&nested_bytes);
            }
            other => {
                return Err(napi::Error::from_reason(format!(
                    "Unsupported struct field type: {other:?}"
                )));
            }
        }
    }

    Ok(buffer)
}

/// Wrap a C function pointer as a callable JS function.
///
/// The returned `JsFunction` captures the function pointer, CIF, type descriptors,
/// and struct definitions in a closure. When called from JS, it marshals each
/// argument to its C representation and invokes the function pointer via libffi.
///
/// # Arguments
///
/// * `env` - The napi environment
/// * `fn_ptr` - The raw C function pointer to wrap
/// * `cb_def` - The callback definition describing the function's signature
/// * `struct_defs` - All struct definitions (for struct-by-value arguments)
/// * `rb_from_bytes_ptr` - Pointer to `rustbuffer_from_bytes` (for RustBuffer args)
/// * `rb_free_ptr` - Pointer to `rustbuffer_free` (reserved for future use)
pub fn create_fn_pointer_wrapper(
    env: &Env,
    fn_ptr: *const c_void,
    cb_def: &CallbackDef,
    struct_defs: &HashMap<String, StructDef>,
    rb_ops: &RustBufferOps,
) -> Result<JsFunction> {
    // Build CIF from the callback definition
    let cif_arg_types: Vec<Type> = cb_def
        .args
        .iter()
        .map(|a| ffi_type_for(a, struct_defs))
        .collect::<napi::Result<Vec<_>>>()?;
    let cif_ret_type = ffi_type_for(&cb_def.ret, struct_defs)?;
    let cif = Cif::new(cif_arg_types, cif_ret_type);

    // Wrap in Rc for capture by the closure (single-threaded napi context)
    let cif = Rc::new(cif);
    let arg_types = Rc::new(cb_def.args.clone());
    let ret_type = cb_def.ret.clone();
    let struct_defs = Rc::new(struct_defs.clone());

    // These are raw pointers but only used on the main thread inside the closure.
    // We wrap them in a newtype to satisfy Send requirements of create_function_from_closure,
    // even though they are only ever called on the main thread.
    let fn_ptr_val = fn_ptr as usize;
    let rb_from_bytes_val = rb_ops.from_bytes_ptr as usize;

    let js_func = env.create_function_from_closure("fn_pointer_wrapper", move |ctx| {
        let fn_ptr = fn_ptr_val as *const c_void;
        let rb_from_bytes_ptr = rb_from_bytes_val as *const c_void;

        // We need to keep boxed values and struct buffers alive until after cif.call().
        // boxed_values owns the heap-allocated scalars; struct_buffers owns the byte arrays.
        let mut boxed_values: Vec<Box<dyn Any>> = Vec::with_capacity(arg_types.len());
        let mut struct_buffers: Vec<Vec<u8>> = Vec::new();
        let mut rb_values: Vec<RustBufferC> = Vec::new();

        // First pass: marshal all JS arguments to their C representations
        for (i, desc) in arg_types.iter().enumerate() {
            let js_val = ctx.get::<JsUnknown>(i)?;

            match desc {
                FfiTypeDesc::Struct(name) => {
                    let js_obj: JsObject = js_val.try_into()?;
                    let struct_def = struct_defs.get(name).ok_or_else(|| {
                        napi::Error::from_reason(format!("Unknown struct type: '{name}'"))
                    })?;
                    let buf = marshal_js_struct_to_bytes(
                        ctx.env,
                        &js_obj,
                        struct_def,
                        &struct_defs,
                        rb_from_bytes_ptr,
                    )?;
                    struct_buffers.push(buf);
                }
                FfiTypeDesc::RustBuffer => {
                    let rb = unsafe {
                        napi_utils::js_uint8array_to_rust_buffer(
                            ctx.env.raw(),
                            js_val,
                            rb_from_bytes_ptr,
                        )?
                    };
                    rb_values.push(rb);
                }
                _ => {
                    let boxed = marshal::js_to_boxed(ctx.env, desc, js_val)?;
                    boxed_values.push(boxed);
                }
            }
        }

        // Second pass: build the Arg array in order, pulling from the correct storage
        let mut ffi_args: Vec<Arg> = Vec::with_capacity(arg_types.len());
        let mut struct_idx = 0;
        let mut rb_idx = 0;
        let mut scalar_idx = 0;

        for desc in arg_types.iter() {
            match desc {
                FfiTypeDesc::Struct(_) => {
                    let buf = &struct_buffers[struct_idx];
                    // SAFETY: buf.as_ptr() points to a valid byte buffer that will remain
                    // alive for the duration of the cif.call() below. libffi reads the
                    // struct data through this pointer.
                    ffi_args.push(unsafe { Arg::new(&*(buf.as_ptr() as *const c_void)) });
                    struct_idx += 1;
                }
                FfiTypeDesc::RustBuffer => {
                    let rb = &rb_values[rb_idx];
                    ffi_args.push(arg(rb));
                    rb_idx += 1;
                }
                _ => {
                    let boxed = &*boxed_values[scalar_idx];
                    ffi_args.push(marshal::boxed_to_arg(desc, boxed)?);
                    scalar_idx += 1;
                }
            }
        }

        // Call the C function pointer via libffi
        let code_ptr = CodePtr::from_ptr(fn_ptr);
        match &ret_type {
            FfiTypeDesc::Void => {
                unsafe { cif.call::<()>(code_ptr, &ffi_args) };
                ctx.env.get_undefined().map(|v| v.into_unknown())
            }
            _ => Err(napi::Error::from_reason(format!(
                "Non-void return types for fn_pointer_wrapper not yet supported: {ret_type:?}"
            ))),
        }
    })?;

    Ok(js_func)
}
