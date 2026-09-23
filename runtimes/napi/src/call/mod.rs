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
//!    or hand off a Rust-owned view for `RustBuffer` returns. The
//!    codegen-emitted lift wrapper consumes the view inside a `try/finally`
//!    and calls back through `rustbuffer_free` to release the underlying Rust
//!    allocation.

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
                // registration time — satisfying `js_uint8array_to_rust_buffer`'s contract.
                let rust_buffer = unsafe {
                    napi_utils::js_uint8array_to_rust_buffer(
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
                // one per call turns that into unbounded growth.
                //
                // The actual trampoline map lives in core, keyed on (callback name, identity).
                // This engine's job is only to mint and stash a stable identity for the JS
                // function object — the Symbol belongs to this `register()` call and so to
                // this env, and never crosses realms — then ask core for the trampoline that
                // identity maps to. A trampoline holds no per-call state, since callbacks
                // receive their handle as an ordinary argument, so reuse across calls is safe.
                //
                // A hit on both lookups costs only that — nothing below runs until a miss.
                //
                // SAFETY: `js_val` is a value from the current callback scope, so `raw()`
                // yields a `napi_value` valid for that scope without transferring ownership.
                let raw_fn_val = unsafe { js_val.raw() };
                // SAFETY: `env` is the active env for this call and `raw_fn_val` is the value
                // read above; a lookup miss is reported as `Ok(None)`, so a JS function
                // carrying no marker is simply minted a fresh identity below rather than
                // misread.
                let stashed = unsafe {
                    registration
                        .trampolines
                        .get(env.raw(), raw_fn_val, cb_name)?
                };
                let cached = stashed.and_then(|identity| module.trampoline_for(cb_name, identity));
                let fn_ptr = match cached {
                    Some(fn_ptr) => fn_ptr,
                    None => {
                        // SAFETY: `raw_fn_val` is a `napi_value` from this callback scope.
                        // `from_raw` errors rather than aborting if it is not a function, and
                        // the declared arg type is `Callback`, so a non-function here is a
                        // caller error surfaced as a JS exception.
                        let js_fn = unsafe { napi::JsFunction::from_raw(env.raw(), raw_fn_val)? };
                        // Minted and stashed here, past the function check but before
                        // anything is built: `set` writes a hidden marker onto the caller's
                        // own value, and a value that turns out not to be a function is
                        // handed back untouched. Stashing first means a build failure below
                        // leaves only an identity with no map entry, which the miss path
                        // above already handles. Building first would leak the trampoline,
                        // its userdata and the strong function ref on a failed `set`, and
                        // leak them again on every retry.
                        let identity = match stashed {
                            Some(identity) => identity,
                            None => {
                                let identity = registration.trampolines.next_identity();
                                // SAFETY: same env and value as the lookup above.
                                unsafe {
                                    registration.trampolines.set(
                                        env.raw(),
                                        raw_fn_val,
                                        cb_name,
                                        identity,
                                    )?
                                };
                                identity
                            }
                        };
                        let user_data = callback::create_callback_user_data(
                            env,
                            js_fn,
                            cb_name,
                            module,
                            registration,
                        )?;
                        let fn_ptr = module
                            .make_callback_trampoline(
                                cb_name,
                                callback::on_js_thread,
                                callback::dispatch_to_js_thread,
                                callback::is_js_thread,
                                user_data,
                            )
                            .map_err(core_err)?;
                        module.remember_trampoline(cb_name, identity, fn_ptr);
                        fn_ptr
                    }
                };
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
            rust_buffer_to_js_uint8array_handoff(
                env,
                rb,
                module.rb_ops().free_ptr,
                &registration.capacity_symbol,
            )
        }
        _ => marshal::read_return_to_js(env, ret_desc, &ret_bytes[..n]),
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
    rb_free_ptr: *const c_void,
    capacity_symbol: &CapacitySymbol,
) -> Result<JsUnknown> {
    let raw_env = env.raw();
    // Until this function hands `rb` to JS, it is the buffer's sole owner: every early
    // return has to release it or the allocation is unreachable.
    //
    // SAFETY (each `free_rustbuffer` below): `rb_free_ptr` was resolved by dlsym at
    // registration time, and `rb` is the buffer the callee just returned, which no other
    // owner holds.
    let len = match usize::try_from(rb.len) {
        Ok(n) => n,
        Err(_) => {
            // SAFETY: as noted above — `rb_free_ptr` was resolved at registration time
            // and `rb` has had no other owner take it yet.
            unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
            return Err(napi::Error::from_reason(
                "RustBuffer len exceeds addressable memory",
            ));
        }
    };

    // Nothing to alias, so any spare capacity has to be released here: a zero-length
    // typed array cannot carry the data pointer forward, leaving `rustbuffer_free`
    // nothing to work from.
    if rb.len == 0 || rb.capacity == 0 || rb.data.is_null() {
        // SAFETY: `raw_env` is valid for this callback scope; a null `data` with length 0
        // allocates an empty buffer without reading anything.
        let typedarray =
            match unsafe { napi_utils::create_uint8array(raw_env, std::ptr::null(), 0) } {
                Ok(typedarray) => typedarray,
                Err(error) => {
                    // SAFETY: as above — `rb_free_ptr` was resolved at registration time
                    // and `rb` has had no other owner take it yet.
                    unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
                    return Err(error);
                }
            };
        // SAFETY: as above — `typedarray` was allocated separately as an empty
        // buffer over `null`, so nothing aliases `rb.data` regardless of which
        // condition tripped the branch; `rb` has had no other owner take it.
        unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
        // SAFETY: `raw_env` is valid for this callback scope, and `typedarray` is the
        // object created just above.
        unsafe { capacity_symbol.set(raw_env, typedarray, 0)? };
        // SAFETY: `raw_env` is valid for this callback scope, and `typedarray` is the
        // object created just above.
        return Ok(unsafe { JsUnknown::from_raw(raw_env, typedarray)? });
    }

    // SAFETY: `rb.data` points to a Rust-owned allocation of at least `len`
    // bytes. We expose it to JS without a finalizer; the codegen-emitted
    // try/finally calls `rustbuffer_free(view)` which will hand the (ptr,
    // capacity) tuple back to the library's `rustbuffer_free`.
    let typedarray = match unsafe { napi_utils::create_external_uint8array(raw_env, rb.data, len) }
    {
        Ok(typedarray) => typedarray,
        Err(error) => {
            // SAFETY: as above — `rb_free_ptr` was resolved at registration time
            // and `rb` has had no other owner take it yet.
            unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
            return Err(error);
        }
    };

    // Mark every handed-off view, including when `capacity == byteLength`: an absent
    // marker means "not the library's, do not free", so an unmarked view leaks. The value
    // is the true capacity, which may exceed `byteLength` — that is `rb.len`, so
    // converters decoding the whole view see only the message bytes.
    //
    // SAFETY: `raw_env` is valid for this callback scope, and `typedarray` is the view
    // created just above.
    if let Err(error) = unsafe { capacity_symbol.set(raw_env, typedarray, rb.capacity) } {
        // SAFETY: `rb_free_ptr` was resolved at registration time. `typedarray`
        // does alias `rb.data` on this path, but it was created with a no-op
        // finalizer — it never owned the allocation — and it is dropped
        // unreturned, so freeing here is the only thing that releases it.
        unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
        return Err(error);
    }

    // SAFETY: `raw_env` is valid for this callback scope, and `typedarray` is the
    // view created above, now marked with its capacity.
    Ok(unsafe { JsUnknown::from_raw(raw_env, typedarray)? })
}
