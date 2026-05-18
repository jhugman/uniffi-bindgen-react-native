/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! JS -> Rust call dispatch.
//!
//! Orchestrates one FFI call end-to-end:
//!
//! 1. Create a [`PreparedCall`](uniffi_runtime_core::PreparedCall) for the target function.
//! 2. Walk the JS arguments and marshal each one into the buffer — scalars go
//!    through [`marshal::write_js_to_slot`], while `RustBuffer`, callback, and
//!    VTable-struct arguments are handled inline with type-specific plumbing.
//! 3. Optionally wire up a `RustCallStatus` out-parameter so Rust can report
//!    rich errors back to JS.
//! 4. Call [`Module::call`](uniffi_runtime_core::Module::call) (which guards
//!    against concurrent unload).
//! 5. Convert the typed [`CallReturn`](uniffi_runtime_core::CallReturn) into a
//!    JS value via [`marshal::read_return_to_js`], or hand off a Rust-owned
//!    view for `RustBuffer` returns. The codegen-emitted lift wrapper consumes
//!    the view inside a `try/finally` and calls back through `rustbuffer_free`
//!    to release the underlying Rust allocation.

mod marshal;

use std::ffi::c_void;
use std::sync::Arc;

use napi::{JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::callback;
use crate::callback::vtable;
use crate::core_err;
use crate::napi_utils;
use crate::napi_utils::CapacitySymbol;
use uniffi_runtime_core::ffi_c_types::{RustBufferC, RustCallStatusC};
use uniffi_runtime_core::slot;
use uniffi_runtime_core::CallReturn;
use uniffi_runtime_core::{FfiTypeDesc, Module};

/// Execute a single FFI call for `fn_name` registered in `module`.
///
/// Marshals each JS argument from `ctx` into the [`PreparedCall`], invokes the
/// native function via [`Module::call`], and returns the result as a JS value.
/// If `has_rust_call_status` is set, the final JS argument is treated as a
/// `{ code, errorBuf }` status object that Rust writes error information into.
pub(crate) fn call_ffi_function(
    env: &napi::Env,
    ctx: &napi::CallContext<'_>,
    fn_name: &str,
    module: &Arc<Module>,
    arg_types: &[FfiTypeDesc],
    has_rust_call_status: bool,
    capacity_symbol: &CapacitySymbol,
) -> Result<JsUnknown> {
    let declared_arg_count = arg_types.len();

    let mut call = module.prepare_call(fn_name).map_err(core_err)?;

    for (i, desc) in arg_types.iter().enumerate() {
        let js_val: JsUnknown = ctx.get(i)?;
        let slot = call.arg_slot(i).map_err(core_err)?;
        match desc {
            FfiTypeDesc::RustBuffer => {
                let rust_buffer = unsafe {
                    napi_utils::js_uint8array_to_rust_buffer(
                        env.raw(),
                        js_val,
                        module.rb_ops().from_bytes_ptr,
                    )?
                };
                slot::write_rust_buffer(slot, rust_buffer);
            }
            FfiTypeDesc::Reference(inner) if matches!(inner.as_ref(), FfiTypeDesc::Struct(_)) => {
                let FfiTypeDesc::Struct(struct_name) = inner.as_ref() else {
                    unreachable!("guard ensures inner is Struct");
                };
                let js_obj = unsafe { JsObject::from_raw(env.raw(), js_val.raw())? };
                let struct_ptr = vtable::build_vtable_struct(env, module, struct_name, &js_obj)?;
                slot::write_pointer(slot, struct_ptr);
            }
            FfiTypeDesc::Callback(cb_name) => {
                let js_fn = unsafe { napi::JsFunction::from_raw(env.raw(), js_val.raw())? };
                let user_data = callback::create_callback_user_data(env, js_fn, cb_name, module)?;
                let fn_ptr = module
                    .make_callback_trampoline(
                        cb_name,
                        callback::on_js_thread,
                        callback::dispatch_to_js_thread,
                        callback::is_js_thread,
                        user_data,
                    )
                    .map_err(core_err)?;
                slot::write_pointer(slot, fn_ptr);
            }
            _ => {
                marshal::write_js_to_slot(env, desc, js_val, slot)?;
            }
        }
    }

    let mut rust_call_status = RustCallStatusC::default();
    let mut status_js_obj: Option<JsObject> = None;

    if has_rust_call_status {
        let status_idx = declared_arg_count;
        let js_status: JsObject = ctx.get(status_idx)?;
        let code_val: i32 = js_status.get_named_property("code")?;
        rust_call_status.code = code_val as i8;
        status_js_obj = Some(js_status);

        let status_ptr = &mut rust_call_status as *mut RustCallStatusC;
        if let Some(rcs_slot) = call.rust_call_status_slot() {
            slot::write_pointer(rcs_slot, status_ptr as *const c_void);
        }
    }

    let call_ret = module.call(call).map_err(core_err)?;

    if has_rust_call_status {
        if let Some(mut js_status) = status_js_obj {
            js_status
                .set_named_property("code", env.create_int32(rust_call_status.code as i32)?)?;

            if rust_call_status.code != 0 && !rust_call_status.error_buf_data.is_null() {
                let raw_env = env.raw();

                let error_rb = RustBufferC {
                    capacity: rust_call_status.error_buf_capacity,
                    len: rust_call_status.error_buf_len,
                    data: rust_call_status.error_buf_data,
                };

                match usize::try_from(rust_call_status.error_buf_len) {
                    Ok(len) => {
                        if let Ok(typedarray) = unsafe {
                            napi_utils::create_uint8array(
                                raw_env,
                                rust_call_status.error_buf_data,
                                len,
                            )
                        } {
                            if let Ok(js_uint8array) =
                                unsafe { JsUnknown::from_raw(raw_env, typedarray) }
                            {
                                js_status.set_named_property("errorBuf", js_uint8array)?;
                            } else {
                                #[cfg(debug_assertions)]
                                eprintln!(
                                    "uniffi-runtime-napi: failed to wrap error buffer as JsUnknown"
                                );
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "uniffi-runtime-napi: failed to create Uint8Array for error buffer ({len} bytes)"
                            );
                        }
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "uniffi-runtime-napi: error buffer len {} exceeds addressable memory",
                            rust_call_status.error_buf_len
                        );
                    }
                }

                unsafe { napi_utils::free_rustbuffer(error_rb, module.rb_ops().free_ptr) };
            }
        }
    }

    match &call_ret {
        CallReturn::RustBuffer(rb) => {
            rust_buffer_to_js_uint8array_handoff(env, *rb, capacity_symbol)
        }
        _ => marshal::read_return_to_js(env, &call_ret),
    }
}

/// Hand a returned `RustBufferC` to JS as a `Uint8Array` view aliasing the
/// Rust-owned bytes — no boundary copy. The codegen-emitted lift wrapper is
/// expected to call `converter.lift(view)` inside a `try/finally` and invoke
/// the runtime's `rustbuffer_free(view)` afterwards. The single mandatory copy
/// now happens inside `lift()` itself (via `dest.set(view)` for byte arrays,
/// `TextDecoder.decode` for strings, field-by-field reads for composites).
///
/// The view's `byteLength` is `rb.len` (so string/raw-byte-array converters
/// that decode the whole view see only the message bytes). Rust may have
/// allocated `rb.capacity > rb.len` bytes, so we stash `capacity` on the view
/// via the runtime's per-registration capacity Symbol; the runtime's
/// `rustbuffer_free` reads it back when releasing the allocation.
///
/// On any error in the handoff, we free the buffer to avoid leaking the
/// Rust-side allocation.
fn rust_buffer_to_js_uint8array_handoff(
    env: &napi::Env,
    rb: RustBufferC,
    capacity_symbol: &CapacitySymbol,
) -> Result<JsUnknown> {
    let raw_env = env.raw();
    let len = match usize::try_from(rb.len) {
        Ok(n) => n,
        Err(_) => {
            return Err(napi::Error::from_reason(
                "RustBuffer len exceeds addressable memory",
            ));
        }
    };

    // Empty RustBuffer (capacity == 0 or null data): no allocation to alias,
    // and no capacity hint needed — the runtime's `rustbuffer_free` short-
    // circuits on empty views without a hint.
    if rb.capacity == 0 || rb.data.is_null() {
        let typedarray = unsafe { napi_utils::create_uint8array(raw_env, std::ptr::null(), 0)? };
        return Ok(unsafe { JsUnknown::from_raw(raw_env, typedarray)? });
    }

    // SAFETY: `rb.data` points to a Rust-owned allocation of at least `len`
    // bytes. We expose it to JS without a finalizer; the codegen-emitted
    // try/finally calls `rustbuffer_free(view)` which will hand the (ptr,
    // capacity) tuple back to the library's `rustbuffer_free`.
    let typedarray = unsafe { napi_utils::create_external_uint8array(raw_env, rb.data, len)? };

    // If `capacity > len`, the view's `byteLength` (== len) under-reports the
    // allocation size. Stash the true capacity so `rustbuffer_free(view)` can
    // free against the original `Layout`. When `capacity == len`, the runtime
    // can recover capacity from `byteLength`, so we skip the property write.
    if rb.capacity != rb.len {
        unsafe { capacity_symbol.set(raw_env, typedarray, rb.capacity)? };
    }

    Ok(unsafe { JsUnknown::from_raw(raw_env, typedarray)? })
}
