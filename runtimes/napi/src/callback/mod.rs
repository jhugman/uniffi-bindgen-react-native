/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # Callback trampolines: routing C function calls back into JavaScript
//!
//! This module provides the NAPI-side implementations of the three function pointers
//! that core's `make_callback_trampoline` invokes:
//!
//! - [`on_js_thread`]: runs on the JS thread, reads args from a flat byte buffer,
//!   converts to JS values, calls the JS function, and writes the return value.
//! - [`dispatch_to_js_thread`]: runs off the JS thread, copies the arg buffer,
//!   sends it to the JS thread via a ThreadsafeFunction, and blocks on a
//!   sync_channel for the return value.
//! - [`is_js_thread`]: returns whether the current thread is the JS main thread.
//!
//! Each callback closure is associated with a [`CallbackUserData`] struct that is
//! leaked to a stable address and passed as `user_data: *const c_void` through the
//! core trampoline protocol.
//!
//! ## Core trampoline protocol
//!
//! ```text
//! Rust library calls fn_ptr
//!   -> libffi invokes core::trampoline_body
//!     -> checks unloading flag
//!     -> packs libffi args into ArgLayout byte buffer (args_buf)
//!     -> if is_js_thread(user_data): on_js_thread(args_buf, ret_buf, user_data)
//!     -> else: dispatch(on_js_thread, args_buf, ret_buf, user_data)
//!     -> copies ret_buf into libffi return slot
//! ```
//!
//! This module handles BOTH simple callbacks (fire-and-forget) AND VTable callbacks
//! (blocking with return values and RustCallStatus handling).

use std::ffi::c_void;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};

use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Env, NapiRaw, NapiValue};

mod marshal;
pub(crate) mod vtable;

use crate::napi_utils;
use uniffi_runtime_core::ffi_c_types::RustBufferC;
use uniffi_runtime_core::slot;
use uniffi_runtime_core::{ArgLayout, FfiTypeDesc, Module};

// ---------------------------------------------------------------------------
// RustCallStatusForVTable
// ---------------------------------------------------------------------------

/// A minimal `#[repr(C)]` view of `RustCallStatus` containing the `code` and
/// `error_buf` fields. Because `code` is the first field, casting a
/// `*mut RustCallStatus` to `*mut RustCallStatusForVTable` is layout-compatible.
#[repr(C)]
struct RustCallStatusForVTable {
    code: i8,
    error_buf: RustBufferC,
}

// ---------------------------------------------------------------------------
// CallbackUserData
// ---------------------------------------------------------------------------

/// Per-callback state passed to core's trampoline as the `user_data` pointer.
///
/// An instance is created when a callback closure is registered and leaked via
/// `Box::into_raw` so it lives for the lifetime of the closure (effectively
/// `'static`). The core trampoline receives it as `*const c_void` on every
/// invocation and passes it through to `on_js_thread`, `dispatch_to_js_thread`,
/// and `is_js_thread`.
struct CallbackUserData {
    /// Raw napi environment handle. Only valid on the main thread.
    raw_env: napi::sys::napi_env,
    /// GC-preventing reference to the JS callback function. Only valid on the main thread.
    fn_ref: napi::sys::napi_ref,
    /// Precomputed layout for the arg byte buffer produced by core's trampoline.
    arg_layout: ArgLayout,
    /// FFI type descriptors for each declared positional argument.
    arg_types: Vec<FfiTypeDesc>,
    /// The FFI type of the callback's return value.
    ret_type: FfiTypeDesc,
    /// Whether the callback signature includes a trailing `RustCallStatus` pointer.
    has_rust_call_status: bool,
    /// Whether the return value is passed via an out-pointer argument rather than
    /// the C return value.
    out_return: bool,
    /// Precomputed size of the return value in bytes (0 for void or out_return).
    ret_size: usize,
    /// Thread-safe function for dispatching to the main thread.
    tsfn: Mutex<Option<ThreadsafeFunction<DispatchPayload, ErrorStrategy::Fatal>>>,
    /// Reference to the Module, needed for fn_pointer wrapping (Callback-typed args).
    module: Arc<Module>,
}

// SAFETY: `CallbackUserData` is shared across threads. This is sound because:
// - `raw_env` and `fn_ref` are only dereferenced on the main thread (the
//   same-thread path checks `is_main_thread()` before touching them).
// - `tsfn` is designed for cross-thread use and protected by a Mutex.
// - `arg_types`, `ret_type`, `arg_layout`, `ret_size` are immutable after construction.
// - `module` is an `Arc<Module>` which is Send+Sync.
unsafe impl Send for CallbackUserData {}
unsafe impl Sync for CallbackUserData {}

// ---------------------------------------------------------------------------
// DispatchPayload
// ---------------------------------------------------------------------------

/// Payload sent from the calling thread to the JS thread via ThreadsafeFunction.
/// Payload sent from the calling thread to the JS thread via ThreadsafeFunction.
struct DispatchPayload {
    /// Copied arg bytes from the core trampoline's flat buffer.
    args: Vec<u8>,
    /// Number of bytes expected in the return buffer.
    ret_len: usize,
    /// Channel to send the return bytes back to the calling thread.
    reply: SyncSender<Vec<u8>>,
    /// The callback's user_data pointer, forwarded so the TSFN handler can
    /// call `on_js_thread`.
    user_data: *const c_void,
}

// SAFETY: The `user_data` pointer is a leaked `Box<CallbackUserData>` with a
// stable address that outlives any dispatch. The `args` Vec owns its data.
unsafe impl Send for DispatchPayload {}

// ---------------------------------------------------------------------------
// The three extern "C" fn pointers
// ---------------------------------------------------------------------------

/// Same-thread fast path: reads args from the flat byte buffer, converts to JS
/// values, calls the JS function, and writes the return value.
///
/// # Safety
///
/// - Must be called on the main Node.js thread.
/// - `args` must point to a valid byte buffer laid out according to the
///   `CallbackUserData::arg_layout`.
/// - `ret` must point to a buffer of at least `ret_size` bytes (as computed by
///   core's `return_size`), or may be ignored when `out_return` is true.
/// - `user_data` must be a valid `*const CallbackUserData` obtained from
///   `Box::into_raw`.
pub extern "C" fn on_js_thread(args: *const u8, ret: *mut u8, user_data: *const c_void) {
    // SAFETY: `user_data` was created via `Box::into_raw` and leaked.
    let ud = unsafe { &*(user_data as *const CallbackUserData) };

    if crate::is_shutting_down() {
        // Zero out the return buffer so the caller gets a deterministic value.
        if !ret.is_null() {
            let ret_size = ud.ret_size;
            if ret_size > 0 {
                unsafe { std::ptr::write_bytes(ret, 0, ret_size) };
            }
        }
        return;
    }

    // SAFETY: We are on the main thread, so raw_env is valid.
    let env = unsafe { Env::from_raw(ud.raw_env) };

    // Resolve the JS function from the persistent reference.
    let mut raw_fn: napi::sys::napi_value = std::ptr::null_mut();
    let status = unsafe { napi::sys::napi_get_reference_value(ud.raw_env, ud.fn_ref, &mut raw_fn) };
    if status != napi::sys::Status::napi_ok || raw_fn.is_null() {
        #[cfg(debug_assertions)]
        eprintln!(
            "uniffi-runtime-napi: callback on_js_thread failed to resolve JS function reference"
        );
        return;
    }

    let js_fn = match unsafe { napi::JsFunction::from_raw(ud.raw_env, raw_fn) } {
        Ok(f) => f,
        Err(_) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "uniffi-runtime-napi: callback on_js_thread failed to reconstruct JsFunction"
            );
            return;
        }
    };

    let declared_count = ud.arg_types.len();
    let mut js_args: Vec<napi::JsUnknown> = Vec::with_capacity(declared_count + 1);

    for (i, desc) in ud.arg_types.iter().enumerate() {
        let slot = &ud.arg_layout.arg_slots[i];
        // SAFETY: `args` points to a buffer of `arg_layout.total_size` bytes.
        // `slot.offset` and `slot.size` are within bounds.
        let arg_bytes = unsafe { std::slice::from_raw_parts(args.add(slot.offset), slot.size) };
        let js_val = match read_arg_bytes_to_js(&env, desc, arg_bytes, &ud.module) {
            Ok(v) => v,
            Err(_e) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "uniffi-runtime-napi: callback on_js_thread failed to convert arg {i} ({desc:?}) to JS: {_e}"
                );
                return;
            }
        };
        js_args.push(js_val);
    }

    // When out_return is true, there is an extra arg in the arg_layout at position
    // `arg_types.len()` which is the out-return pointer.
    let mut out_return_ptr: *mut c_void = std::ptr::null_mut();
    if ud.out_return {
        let out_slot = &ud.arg_layout.arg_slots[declared_count];
        // SAFETY: The slot holds a pointer value.
        unsafe {
            let ptr_bytes = std::slice::from_raw_parts(args.add(out_slot.offset), out_slot.size);
            out_return_ptr = slot::read_pointer(ptr_bytes) as *mut c_void;
        }
    }

    // Extract the RustCallStatus pointer if present.
    let mut status_ptr: *mut RustCallStatusForVTable = std::ptr::null_mut();
    if ud.has_rust_call_status {
        if let Some(ref rcs_slot) = ud.arg_layout.rust_call_status_slot {
            unsafe {
                let ptr_bytes =
                    std::slice::from_raw_parts(args.add(rcs_slot.offset), rcs_slot.size);
                status_ptr = slot::read_pointer(ptr_bytes) as *mut RustCallStatusForVTable;
            }
        }
    }

    // When has_rust_call_status && !out_return: create a {code} JS status object
    // and append to js_args (pass-by-reference status protocol).
    if ud.has_rust_call_status && !ud.out_return {
        let code = if !status_ptr.is_null() {
            unsafe { (*status_ptr).code as i32 }
        } else {
            0
        };

        let Ok(mut js_status) = env.create_object() else {
            return;
        };
        let Ok(code_val) = env.create_int32(code) else {
            return;
        };
        if js_status.set_named_property("code", code_val).is_err() {
            return;
        }
        js_args.push(js_status.into_unknown());
    }

    let call_result = js_fn.call(None, &js_args);

    if ud.out_return {
        // out_return path: the return is via the out-pointer, not the ret buffer.
        if let Ok(js_ret) = call_result {
            if ud.has_rust_call_status {
                // UniffiResult protocol: JS returns { code, pointee?, errorBuf? }
                unsafe {
                    if let Ok(result_obj) = napi::JsObject::from_raw(ud.raw_env, js_ret.raw()) {
                        write_uniffi_result(
                            &env,
                            &result_obj,
                            status_ptr,
                            out_return_ptr,
                            &ud.ret_type,
                            &ud.module,
                        );
                    }
                }
            } else if !out_return_ptr.is_null() {
                // Direct struct return (no UniffiResult wrapping).
                // Marshal the JS return value to bytes and write to the out-return pointer.
                unsafe {
                    write_js_value_to_pointer(
                        &env,
                        &ud.ret_type,
                        js_ret,
                        out_return_ptr as *mut u8,
                        &ud.module,
                    );
                }
            }
        }
    } else {
        // Non-out_return path: write return to `ret` buffer.

        // Read back mutated status code from the JS status object (pass-by-reference).
        if ud.has_rust_call_status && !status_ptr.is_null() {
            if let Some(js_status_unknown) = js_args.last() {
                unsafe {
                    if let Ok(js_status_obj) =
                        napi::JsObject::from_raw(ud.raw_env, js_status_unknown.raw())
                    {
                        if let Ok(code_val) = js_status_obj.get_named_property::<i32>("code") {
                            (*status_ptr).code = code_val as i8;
                        }
                        // If code != 0, check for errorBuf and write it.
                        if (*status_ptr).code != 0 {
                            write_error_buf_from_status_obj(
                                &js_status_obj,
                                status_ptr,
                                ud.raw_env,
                                ud.module.rb_ops().from_bytes_ptr,
                            );
                        }
                    }
                }
            }
        }

        if let Ok(js_ret) = call_result {
            if !ret.is_null() {
                let ret_size = ud.ret_size;
                if ret_size > 0 {
                    unsafe {
                        let ret_bytes = std::slice::from_raw_parts_mut(ret, ret_size);
                        let _ = write_js_return_to_bytes(
                            &env,
                            &ud.ret_type,
                            js_ret,
                            ret_bytes,
                            &ud.module,
                        );
                    }
                }
            }
        }
    }
}

/// Cross-thread path: copies the arg buffer, dispatches to the JS thread via
/// ThreadsafeFunction, and blocks until the JS thread sends back the return bytes.
///
/// # Safety
///
/// - Must NOT be called on the main thread (the blocking recv would deadlock).
/// - `on_js_thread_fn` must be a valid function pointer.
/// - `args`, `ret`, and `user_data` follow the same contracts as `on_js_thread`.
pub extern "C" fn dispatch_to_js_thread(
    _on_js_thread_fn: uniffi_runtime_core::OnJsThreadFn,
    args: *const u8,
    ret: *mut u8,
    user_data: *const c_void,
) {
    // SAFETY: `user_data` was created via `Box::into_raw` and leaked.
    let ud = unsafe { &*(user_data as *const CallbackUserData) };

    if crate::is_shutting_down() {
        return;
    }

    // Copy the arg buffer so the calling thread can return its stack frame.
    let args_len = ud.arg_layout.total_size;
    let args_copy = if args_len > 0 && !args.is_null() {
        // SAFETY: `args` points to `args_len` valid bytes.
        unsafe { std::slice::from_raw_parts(args, args_len) }.to_vec()
    } else {
        Vec::new()
    };

    // Compute ret_len: for out_return=true, ret_len=0 (returns are via out-pointer).
    let ret_len = ud.ret_size;

    // Create the rendezvous channel.
    let (tx, rx) = sync_channel(1);

    let payload = DispatchPayload {
        args: args_copy,
        ret_len,
        reply: tx,
        user_data,
    };

    // Lock the tsfn and call it.
    {
        let tsfn_guard = ud.tsfn.lock().expect("tsfn mutex poisoned");
        let Some(tsfn) = tsfn_guard.as_ref() else {
            #[cfg(debug_assertions)]
            eprintln!("uniffi-runtime-napi: dispatch_to_js_thread has no ThreadsafeFunction");
            return;
        };
        tsfn.call(payload, ThreadsafeFunctionCallMode::Blocking);
    }

    // Block until the JS thread sends back the return bytes.
    match rx.recv() {
        Ok(ret_bytes) => {
            if ret_len > 0 && !ret.is_null() && !ret_bytes.is_empty() {
                let copy_len = ret_len.min(ret_bytes.len());
                // SAFETY: `ret` points to at least `ret_len` bytes.
                unsafe {
                    std::ptr::copy_nonoverlapping(ret_bytes.as_ptr(), ret, copy_len);
                }
            }
        }
        Err(_) => {
            // The sender was dropped (e.g., shutdown). Zero the return buffer.
            if ret_len > 0 && !ret.is_null() {
                unsafe { std::ptr::write_bytes(ret, 0, ret_len) };
            }
        }
    }
}

/// Returns whether the current thread is the main JS thread.
///
/// # Safety
///
/// `user_data` is unused by this implementation (the check is global), but must
/// be a valid pointer per the core trampoline protocol.
pub extern "C" fn is_js_thread(_user_data: *const c_void) -> bool {
    crate::is_main_thread()
}

// ---------------------------------------------------------------------------
// create_callback_user_data
// ---------------------------------------------------------------------------

/// Create a [`CallbackUserData`] for a callback, suitable for passing to
/// `Module::make_callback_trampoline` as the `user_data` pointer.
///
/// This function:
/// 1. Creates a persistent `napi_ref` for the JS function.
/// 2. Looks up the callback def from the module spec.
/// 3. Computes the `ArgLayout` (including extra pointer arg for out_return).
/// 4. Creates a `ThreadsafeFunction` for cross-thread dispatch.
/// 5. Leaks the `CallbackUserData` and returns it as `*const c_void`.
pub fn create_callback_user_data(
    env: &Env,
    js_fn: napi::JsFunction,
    callback_name: &str,
    module: &Arc<Module>,
) -> napi::Result<*const c_void> {
    let def = module
        .spec_callbacks()
        .get(callback_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Unknown callback: {callback_name}")))?;

    // Compute ArgLayout. When out_return is true, we must include an extra
    // VoidPointer arg (for the out-return pointer) between the declared args
    // and the RustCallStatus slot.
    let mut layout_args = def.args.clone();
    if def.out_return {
        layout_args.push(FfiTypeDesc::VoidPointer);
    }
    let arg_layout =
        ArgLayout::compute(&layout_args, def.has_rust_call_status).map_err(crate::core_err)?;

    // Precompute the return value size.
    let ret_size = if def.out_return {
        0
    } else {
        match &def.ret {
            FfiTypeDesc::Void => 0,
            other => uniffi_runtime_core::slot_size_align(other)
                .map(|(s, _)| s)
                .unwrap_or(0),
        }
    };

    // Create a GC-preventing reference to the JS function.
    let mut fn_ref: napi::sys::napi_ref = std::ptr::null_mut();
    let ref_status =
        unsafe { napi::sys::napi_create_reference(env.raw(), js_fn.raw(), 1, &mut fn_ref) };
    if ref_status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_reason(format!(
            "Failed to create reference for callback '{callback_name}'"
        )));
    }

    // Allocate the userdata and leak it to a stable address.
    let userdata = Box::new(CallbackUserData {
        raw_env: env.raw(),
        fn_ref,
        arg_layout,
        arg_types: def.args.clone(),
        ret_type: def.ret.clone(),
        has_rust_call_status: def.has_rust_call_status,
        out_return: def.out_return,
        ret_size,
        tsfn: Mutex::new(None),
        module: Arc::clone(module),
    });
    let userdata_ptr = Box::into_raw(userdata);

    // Create a ThreadsafeFunction for cross-thread dispatch.
    // The TSFN callback will call `on_js_thread` with the payload's args.
    let noop_fn = env.create_function_from_closure("cb_tsfn_dispatch", |_ctx| Ok(()))?;
    let tsfn: ThreadsafeFunction<DispatchPayload, ErrorStrategy::Fatal> = noop_fn
        .create_threadsafe_function(
            0,
            move |ctx: napi::threadsafe_function::ThreadSafeCallContext<DispatchPayload>| {
                let payload = ctx.value;

                let mut ret_buf = vec![0u8; payload.ret_len];
                // Call on_js_thread to do the actual JS call.
                on_js_thread(
                    payload.args.as_ptr(),
                    ret_buf.as_mut_ptr(),
                    payload.user_data,
                );
                // Send the return bytes back to the calling thread.
                let _ = payload.reply.send(ret_buf);

                // Return empty vec—the TSFN callback mechanism requires a Vec<JsUnknown>
                // but we've handled everything ourselves.
                Ok(Vec::<napi::JsUnknown>::new())
            },
        )?;
    let mut tsfn = tsfn;

    // Register the raw TSFN handle so the env cleanup hook can abort it at shutdown.
    crate::register_tsfn(tsfn.raw());

    // Unref the TSFN so it does not prevent the Node.js event loop from exiting.
    tsfn.unref(env)?;

    // Store the TSFN in the userdata.
    // SAFETY: `userdata_ptr` is valid and uniquely owned. We are still on the main
    // thread; no concurrent access is possible yet.
    unsafe {
        let tsfn_slot = &mut (*userdata_ptr).tsfn;
        *tsfn_slot.get_mut().expect("mutex not poisoned") = Some(tsfn);
    }

    Ok(userdata_ptr as *const c_void)
}

// ---------------------------------------------------------------------------
// Byte <-> JS conversion helpers (private)
// ---------------------------------------------------------------------------

/// Read a single arg's bytes from a flat buffer slice and convert to a JS value.
///
/// The `arg_bytes` slice has exactly `slot.size` bytes for the argument.
///
/// # Safety (internal)
///
/// The caller must ensure `arg_bytes` has the correct size for `desc`.
fn read_arg_bytes_to_js(
    env: &Env,
    desc: &FfiTypeDesc,
    arg_bytes: &[u8],
    module: &Arc<Module>,
) -> napi::Result<napi::JsUnknown> {
    let rb_free_ptr = module.rb_ops().free_ptr;
    match desc {
        FfiTypeDesc::UInt8 => Ok(env
            .create_uint32(slot::read_u8(arg_bytes) as u32)?
            .into_unknown()),
        FfiTypeDesc::Int8 => Ok(env
            .create_int32(slot::read_i8(arg_bytes) as i32)?
            .into_unknown()),
        FfiTypeDesc::UInt16 => Ok(env
            .create_uint32(slot::read_u16(arg_bytes) as u32)?
            .into_unknown()),
        FfiTypeDesc::Int16 => Ok(env
            .create_int32(slot::read_i16(arg_bytes) as i32)?
            .into_unknown()),
        FfiTypeDesc::UInt32 => Ok(env.create_uint32(slot::read_u32(arg_bytes))?.into_unknown()),
        FfiTypeDesc::Int32 => Ok(env.create_int32(slot::read_i32(arg_bytes))?.into_unknown()),
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => Ok(env
            .create_bigint_from_u64(slot::read_u64(arg_bytes))?
            .into_unknown()?),
        FfiTypeDesc::Int64 => Ok(env
            .create_bigint_from_i64(slot::read_i64(arg_bytes))?
            .into_unknown()?),
        FfiTypeDesc::Float32 => Ok(env
            .create_double(slot::read_f32(arg_bytes) as f64)?
            .into_unknown()),
        FfiTypeDesc::Float64 => Ok(env.create_double(slot::read_f64(arg_bytes))?.into_unknown()),
        FfiTypeDesc::RustBuffer => {
            let rb = slot::read_rust_buffer(arg_bytes);
            let len = usize::try_from(rb.len).map_err(|_| {
                unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
                napi::Error::from_reason("RustBuffer len exceeds addressable memory")
            })?;
            let raw_env = env.raw();

            // SAFETY: `create_uint8array` copies `rb.data` into a new JS ArrayBuffer.
            // The data is valid because we have not yet freed the buffer.
            let typedarray = unsafe { napi_utils::create_uint8array(raw_env, rb.data, len)? };

            // Free the original RustBuffer—the JS Uint8Array now owns its copy.
            unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };

            Ok(unsafe { napi::JsUnknown::from_raw(raw_env, typedarray)? })
        }
        FfiTypeDesc::Callback(cb_name) => {
            // Read the function pointer from bytes and wrap it as a callable JS function.
            let fn_ptr = slot::read_pointer(arg_bytes) as *const c_void;
            let js_fn = self::marshal::create_fn_pointer_wrapper(env, fn_ptr, cb_name, module)?;
            Ok(js_fn.into_unknown())
        }
        FfiTypeDesc::VoidPointer | FfiTypeDesc::Reference(_) | FfiTypeDesc::MutReference(_) => {
            // Read a pointer-sized value and expose as BigInt.
            let ptr_val = slot::read_pointer(arg_bytes);
            Ok(env.create_bigint_from_u64(ptr_val as u64)?.into_unknown()?)
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported callback arg type: {desc:?}"
        ))),
    }
}

/// Convert a JS return value to bytes and write them into `ret_bytes` slice.
///
/// # Safety (internal)
///
/// The caller must ensure `ret_bytes` has sufficient size for `desc`.
unsafe fn write_js_return_to_bytes(
    env: &Env,
    desc: &FfiTypeDesc,
    js_val: napi::JsUnknown,
    ret_bytes: &mut [u8],
    module: &Arc<Module>,
) -> napi::Result<()> {
    let raw_env = env.raw();
    let rb_from_bytes_ptr = module.rb_ops().from_bytes_ptr;
    match desc {
        FfiTypeDesc::Void => Ok(()),
        FfiTypeDesc::UInt8 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_u8(ret_bytes, num.get_double()? as u8);
            Ok(())
        }
        FfiTypeDesc::Int8 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_i8(ret_bytes, num.get_double()? as i8);
            Ok(())
        }
        FfiTypeDesc::UInt16 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_u16(ret_bytes, num.get_double()? as u16);
            Ok(())
        }
        FfiTypeDesc::Int16 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_i16(ret_bytes, num.get_double()? as i16);
            Ok(())
        }
        FfiTypeDesc::UInt32 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_u32(ret_bytes, num.get_double()? as u32);
            Ok(())
        }
        FfiTypeDesc::Int32 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_i32(ret_bytes, num.get_double()? as i32);
            Ok(())
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            let bigint = napi::JsBigInt::from_raw(raw_env, js_val.raw())?;
            let (v, _) = bigint.get_u64()?;
            slot::write_u64(ret_bytes, v);
            Ok(())
        }
        FfiTypeDesc::Int64 => {
            let bigint = napi::JsBigInt::from_raw(raw_env, js_val.raw())?;
            let (v, _) = bigint.get_i64()?;
            slot::write_i64(ret_bytes, v);
            Ok(())
        }
        FfiTypeDesc::Float32 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_f32(ret_bytes, num.get_double()? as f32);
            Ok(())
        }
        FfiTypeDesc::Float64 => {
            let num = napi::JsNumber::from_raw(raw_env, js_val.raw())?;
            slot::write_f64(ret_bytes, num.get_double()?);
            Ok(())
        }
        FfiTypeDesc::RustBuffer => {
            // Read Uint8Array -> rustbuffer_from_bytes -> write RustBufferC bytes.
            // Truncating copy: caller may pass a `ret_bytes` smaller than RustBufferC
            // when the return slot is a different shape; preserve historical behavior.
            let rb = napi_utils::js_uint8array_to_rust_buffer(raw_env, js_val, rb_from_bytes_ptr)?;
            let rb_bytes = slot::rust_buffer_to_bytes(&rb);
            let copy_len = rb_bytes.len().min(ret_bytes.len());
            ret_bytes[..copy_len].copy_from_slice(&rb_bytes[..copy_len]);
            Ok(())
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported return type for write_js_return_to_bytes: {desc:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Write a JS value to a raw pointer destination. Used for out-return pointers
/// when `out_return && !has_rust_call_status` (direct struct return).
///
/// # Safety
///
/// `dest` must point to a writable buffer of at least `slot_size_align(desc)` bytes.
unsafe fn write_js_value_to_pointer(
    env: &Env,
    desc: &FfiTypeDesc,
    js_val: napi::JsUnknown,
    dest: *mut u8,
    module: &Arc<Module>,
) {
    // For Struct types, use fn_pointer::marshal_js_struct_to_bytes which handles
    // the full C struct layout (slot_size_align doesn't support Struct types).
    if let FfiTypeDesc::Struct(struct_name) = desc {
        let rb_from_bytes_ptr = module.rb_ops().from_bytes_ptr;
        if let Ok(js_obj) = napi::JsObject::from_raw(env.raw(), js_val.raw()) {
            if let Ok(bytes) = self::marshal::marshal_js_struct_to_bytes(
                env,
                &js_obj,
                struct_name,
                module,
                rb_from_bytes_ptr,
            ) {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, bytes.len());
            }
        }
        return;
    }

    let size = match uniffi_runtime_core::slot_size_align(desc) {
        Ok((s, _)) => s,
        Err(_) => return,
    };
    if size == 0 {
        return;
    }
    // Max non-struct size is RustBufferC (24 bytes: {u64, u64, *mut u8}).
    let mut buf = [0u8; std::mem::size_of::<RustBufferC>()];
    debug_assert!(size <= buf.len());
    if write_js_return_to_bytes(env, desc, js_val, &mut buf[..size], module).is_ok() {
        std::ptr::copy_nonoverlapping(buf.as_ptr(), dest, size);
    }
}

/// Handle the UniffiResult protocol for `out_return && has_rust_call_status`.
///
/// Reads `{ code, pointee?, errorBuf? }` from the JS return object and writes
/// to the C status and out-return pointers.
///
/// # Safety
///
/// `status_ptr` must be null or point to a valid `RustCallStatusForVTable`.
/// `out_return_ptr` must be null or point to a writable buffer for the return type.
unsafe fn write_uniffi_result(
    env: &Env,
    result_obj: &napi::JsObject,
    status_ptr: *mut RustCallStatusForVTable,
    out_return_ptr: *mut c_void,
    ret_type: &FfiTypeDesc,
    module: &Arc<Module>,
) {
    let rb_from_bytes_ptr = module.rb_ops().from_bytes_ptr;

    // Read and write the status code.
    if !status_ptr.is_null() {
        if let Ok(code) = result_obj.get_named_property::<i32>("code") {
            (*status_ptr).code = code as i8;
        }

        // If code != 0, check for errorBuf.
        if (*status_ptr).code != 0 {
            if let Ok(error_buf_val) = result_obj.get_named_property::<napi::JsUnknown>("errorBuf")
            {
                // Check if the value is a TypedArray (not undefined/null).
                if let Some((data, length)) =
                    napi_utils::read_typedarray_data(env.raw(), error_buf_val.raw())
                {
                    if length > 0 && !data.is_null() {
                        if let Ok(rb) =
                            napi_utils::rustbuffer_from_raw_bytes(data, length, rb_from_bytes_ptr)
                        {
                            (*status_ptr).error_buf = rb;
                        }
                    }
                }
            }
        }
    }

    // Write the pointee if present and status is success.
    if !out_return_ptr.is_null() {
        let code = if !status_ptr.is_null() {
            (*status_ptr).code
        } else {
            0
        };
        // Only write pointee when code == 0 (success).
        if code == 0 {
            if let Ok(pointee) = result_obj.get_named_property::<napi::JsUnknown>("pointee") {
                write_js_value_to_pointer(
                    env,
                    ret_type,
                    pointee,
                    out_return_ptr as *mut u8,
                    module,
                );
            }
        }
    }
}

/// Write errorBuf from a JS status object to the C RustCallStatus.
///
/// # Safety
///
/// `status_ptr` must point to a valid `RustCallStatusForVTable`.
unsafe fn write_error_buf_from_status_obj(
    js_status_obj: &napi::JsObject,
    status_ptr: *mut RustCallStatusForVTable,
    raw_env: napi::sys::napi_env,
    rb_from_bytes_ptr: *const c_void,
) {
    if status_ptr.is_null() {
        return;
    }
    if let Ok(error_buf_val) = js_status_obj.get_named_property::<napi::JsUnknown>("errorBuf") {
        if let Some((data, length)) = napi_utils::read_typedarray_data(raw_env, error_buf_val.raw())
        {
            if length > 0 && !data.is_null() {
                if let Ok(rb) =
                    napi_utils::rustbuffer_from_raw_bytes(data, length, rb_from_bytes_ptr)
                {
                    (*status_ptr).error_buf = rb;
                }
            }
        }
    }
}
