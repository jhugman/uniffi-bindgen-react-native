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
//! 4. Call [`Module::call`](uniffi_runtime_core::Module::call) (which
//!    guards against concurrent unload) to write the return's native-endian
//!    bytes into a buffer sized from the function's own return descriptor.
//! 5. Convert those bytes into a JS value via [`marshal::read_return_to_js`],
//!    or hand a `RustBuffer` return to JS via [`crate::buffers::BufferViews::hand_off`]. The
//!    codegen-emitted lift wrapper consumes the view inside a `try/finally`
//!    and calls back through `rustbuffer_free` to release the underlying Rust
//!    allocation, if the view still aliases it.

mod marshal;

use std::ffi::c_void;
use std::sync::Arc;

use napi::{JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::buffers::BufferViews;
use crate::callback;
use crate::callback::vtable;
use crate::core_err;
use crate::napi_utils;
use uniffi_runtime_core::ffi_c_types::{RustBufferC, RustCallStatusC};
use uniffi_runtime_core::slot;
use uniffi_runtime_core::{FfiTypeDesc, Module};

/// Execute a single FFI call for `fn_name` registered in `module`.
///
/// Marshals each JS argument from `ctx` into the [`PreparedCall`], invokes the
/// native function via [`Module::call`], and returns the result as a JS value.
/// If `has_rust_call_status` is set, the final JS argument is treated as a
/// `{ code, errorBuf }` status object that Rust writes error information into.
///
/// `ret_desc`/`ret_size` are resolved once at registration alongside
/// `arg_types`, so nothing on this path looks the function up a second time.
///
/// A `RustBuffer` return is handed off by `ret_views`, which is the registration's
/// own views except for a future's `complete`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn call_ffi_function(
    env: &napi::Env,
    ctx: &napi::CallContext<'_>,
    fn_name: &str,
    module: &Arc<Module>,
    arg_types: &[FfiTypeDesc],
    ret_desc: &FfiTypeDesc,
    ret_size: usize,
    has_rust_call_status: bool,
    registration: &Arc<crate::register::Registration>,
    ret_views: &BufferViews,
) -> Result<JsUnknown> {
    let declared_arg_count = arg_types.len();

    let mut call = module.prepare_call(fn_name).map_err(core_err)?;

    // NOTE: arguments are lowered in order, and lowering a library-owned `RustBuffer` adopts its
    // allocation (the callee frees it). If a *later* argument fails to lower we return early
    // without invoking the callee, so any already-adopted buffer in this call is orphaned. This
    // only happens on a misuse/error path — e.g. passing the same alloc'd view twice, which trips
    // the "already consumed" guard — never on the happy path, which always reaches the call.
    for (i, desc) in arg_types.iter().enumerate() {
        let js_val: JsUnknown = ctx.get(i)?;
        let slot = call.arg_slot(i).map_err(core_err)?;
        match desc {
            FfiTypeDesc::RustBuffer => {
                // SAFETY: `env` is the active env for this call; `js_val` is the argument
                // value just read from `ctx`, and `from_bytes_ptr` was resolved at
                // registration time — satisfying `BufferViews::lower`'s contract.
                let rust_buffer = unsafe {
                    registration.buffer_views.lower(
                        env.raw(),
                        js_val,
                        module.rb_ops().from_bytes_ptr,
                        &registration.capacity_symbol,
                    )?
                };
                slot::write_rust_buffer(slot, rust_buffer);
            }
            FfiTypeDesc::Reference(inner) if matches!(inner.as_ref(), FfiTypeDesc::Struct(_)) => {
                let FfiTypeDesc::Struct(struct_name) = inner.as_ref() else {
                    unreachable!("guard ensures inner is Struct");
                };
                // SAFETY: `env` is the active env for this call; `js_val` is the argument
                // value just read from `ctx`.
                let js_obj = unsafe { JsObject::from_raw(env.raw(), js_val.raw())? };
                let struct_ptr =
                    vtable::build_vtable_struct(env, module, struct_name, &js_obj, registration)?;
                slot::write_pointer(slot, struct_ptr);
            }
            FfiTypeDesc::Callback(cb_name) => {
                // Reuse this function's trampoline if it already has one. Building one is
                // permanently leaked by design — the library may invoke the pointer from any
                // thread later — so the intended bound is one per callback type, and building
                // one per call turns that into unbounded growth. A non-function here is a
                // caller error, surfaced as a JS exception.
                let fn_ptr = callback::cache::trampoline_for_js_fn(
                    env,
                    &js_val,
                    cb_name,
                    module,
                    registration,
                )?;
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

    // A return is never wider than a RustBuffer, so it fits a stack buffer. This
    // runs on every call, so it must not allocate.
    let mut ret_buf = [0u8; std::mem::size_of::<RustBufferC>()];
    debug_assert!(ret_size <= ret_buf.len(), "return wider than RustBufferC");
    let ret_bytes = &mut ret_buf[..ret_size];
    let n = module.call(call, ret_bytes).map_err(core_err)?;

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
                        // SAFETY: `raw_env` is valid for this call scope, and
                        // `error_buf_data` points to at least `len` bytes owned by the
                        // callee's error RustBuffer.
                        if let Ok(typedarray) = unsafe {
                            napi_utils::create_uint8array(
                                raw_env,
                                rust_call_status.error_buf_data,
                                len,
                            )
                        } {
                            if let Ok(js_uint8array) =
                                // SAFETY: `raw_env` is valid for this call scope, and
                                // `typedarray` is the value just created above.
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

                // SAFETY: `free_ptr` was resolved at registration time; `error_rb`
                // mirrors the callee's error RustBuffer fields, which nothing else
                // has taken ownership of.
                unsafe { napi_utils::free_rustbuffer(error_rb, module.rb_ops().free_ptr) };
            }
        }
    }

    match ret_desc {
        FfiTypeDesc::RustBuffer => {
            let rb_size = std::mem::size_of::<RustBufferC>();
            // `n` bytes are what `call` actually wrote; requiring both it and the
            // backing allocation to cover `rb_size` is what lets the SAFETY note below
            // hold even if either one comes back short.
            if n < rb_size || ret_bytes.len() < rb_size {
                return Err(marshal::short_return(rb_size, n.min(ret_bytes.len())));
            }
            // SAFETY: `ret_desc` is `RustBuffer`, so `call` wrote exactly
            // `size_of::<RustBufferC>()` bytes at the front of `ret_bytes`, and the
            // check above confirms both `n` and the allocation cover that range.
            let rb: RustBufferC = unsafe { std::ptr::read_unaligned(ret_bytes.as_ptr().cast()) };
            ret_views.hand_off(
                env,
                rb,
                module.rb_ops().free_ptr,
                &registration.capacity_symbol,
            )
        }
        _ => marshal::read_return_to_js(env, ret_desc, &ret_bytes[..n]),
    }
}
