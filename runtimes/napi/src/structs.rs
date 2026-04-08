/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # VTables: the Object Protocol
//!
//! UniFFI represents trait implementations as *VTables* — C structs whose fields are
//! function pointers. When JavaScript implements a UniFFI trait, we must construct a
//! C-compatible struct where each field is a function pointer that, when called by Rust,
//! routes to the corresponding JS method. This module builds those structs.
//!
//! ## The Three Layers
//!
//! Each VTable callback involves three layers of indirection:
//!
//! 1. **The C caller** invokes a function pointer stored in the VTable struct.
//! 2. **libffi** routes that call to our [`vtable_trampoline_callback`].
//! 3. **The trampoline** either calls JS directly (same-thread) or dispatches via
//!    `ThreadsafeFunction` (cross-thread).
//!
//! ## Cross-thread dispatch with return values
//!
//! Unlike simple callbacks (fire-and-forget), VTable methods often return values. The
//! cross-thread path must therefore be *blocking*: it serializes the arguments into a
//! [`VTableCallRequest`], sends it to the main thread via a `ThreadsafeFunction`, and
//! then *waits* on a `sync_channel` for the [`VTableCallResponse`] containing the
//! return value. This is a rendezvous pattern — the calling thread blocks until JS
//! produces the answer.
//!
//! ## The `ffi_arg` widening convention
//!
//! When a libffi closure returns a value, integer types smaller than the machine word
//! must be widened to `ffi_arg` (unsigned) or `ffi_sarg` (signed). This is a libffi
//! convention documented in the libffi manual, not a Rust quirk. The
//! [`write_return_value`] and [`write_raw_return_value`] functions implement this
//! widening when called with `widen_to_ffi_arg: true`. `Float32` and `Float64` are
//! written at their natural width. Types >= 64
//! bits (`Int64`, `UInt64`, `RustBuffer`) are also written at natural width since they
//! are already at least as wide as `ffi_arg` on 64-bit platforms.
//!
//! ## `RustCallStatusForVTable`
//!
//! A minimal `#[repr(C)]` struct containing only the `code: i8` field. The full
//! `RustCallStatus` has an `error_buf` (`RustBuffer`) following the code, but the
//! trampoline only needs to read/write the code field. Because `code` is the first
//! field, this partial view is layout-compatible — we can safely cast a
//! `*mut RustCallStatus` to `*mut RustCallStatusForVTable` and access the code.
//!
//! ## Lifetime management
//!
//! The userdata, closures, and `Vec` backing the VTable struct are all intentionally
//! leaked (`Box::into_raw` + `mem::forget`). This is necessary because the Rust library
//! holds function pointers into these allocations and may call them at any time from any
//! thread. There is no mechanism to know when the library discards a VTable, so the
//! allocations live for the process lifetime. The `napi_ref` preventing GC of the JS
//! functions is similarly permanent.

use std::collections::HashMap;
use std::ffi::c_void;

use libffi::low;
use libffi::middle::{Cif, Closure, Type};
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Env, JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::callback::{
    c_arg_to_js, js_return_to_raw, raw_arg_to_js, read_raw_arg, CallbackDef, RawCallbackArg,
};
use crate::cif::ffi_type_for;
use crate::ffi_c_types::{RustBufferC, RustBufferOps};
use crate::ffi_type::FfiTypeDesc;
use crate::fn_pointer;
use crate::is_main_thread;
use crate::napi_utils;

/// Packed arguments for cross-thread VTable callback dispatch.
///
/// When a VTable method is called from a non-JS thread, we cannot touch JS values
/// directly. Instead, the calling thread reads the C arguments into portable Rust
/// types ([`RawCallbackArg`]), bundles them into this struct, and sends it to the
/// JS thread via a `ThreadsafeFunction`. If a return value is needed, the
/// `response_tx` channel provides the rendezvous point where the calling thread
/// blocks until JS produces the answer.
struct VTableCallRequest {
    /// C argument values, read from raw pointers on the calling thread.
    args: Vec<RawCallbackArg>,
    /// If has_rust_call_status, the initial code value from C.
    rust_call_status_code: i8,
    /// Channel to send the result back to the calling thread.
    /// `None` for fire-and-forget (non-blocking) callbacks such as the free callback.
    response_tx: Option<std::sync::mpsc::SyncSender<VTableCallResponse>>,
}

/// Result sent back from the JS thread to the calling thread.
struct VTableCallResponse {
    /// Return value (using RawCallbackArg for type-safe transport).
    return_value: Option<RawCallbackArg>,
    /// Updated RustCallStatus code from JS.
    rust_call_status_code: i8,
}

/// A single field in a struct definition.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub field_type: FfiTypeDesc,
}

/// A parsed struct definition (list of fields).
#[derive(Debug, Clone)]
pub struct StructDef {
    pub fields: Vec<StructField>,
}

/// Parse the `structs` map from JS definitions.
/// Each struct is an array of { name, type } objects.
pub fn parse_structs(definitions: &JsObject) -> Result<HashMap<String, StructDef>> {
    let mut map = HashMap::new();

    let has_structs: bool = definitions.has_named_property("structs")?;
    if !has_structs {
        return Ok(map);
    }

    let structs: JsObject = definitions.get_named_property("structs")?;
    let names = structs.get_property_names()?;
    let len = names.get_array_length()?;

    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let fields_arr: JsObject = structs.get_named_property(&name)?;
        let fields_len = fields_arr.get_array_length()?;
        let mut fields = Vec::with_capacity(fields_len as usize);

        for j in 0..fields_len {
            let field_obj: JsObject = fields_arr.get_element(j)?;
            let field_name: String = field_obj.get_named_property("name")?;
            let type_obj: JsObject = field_obj.get_named_property("type")?;
            let field_type = FfiTypeDesc::from_js_object(&type_obj)?;
            fields.push(StructField {
                name: field_name,
                field_type,
            });
        }

        map.insert(name, StructDef { fields });
    }

    Ok(map)
}

/// Userdata for VTable callback trampolines.
///
/// Unlike [`crate::callback::TrampolineUserdata`] (used for simple callbacks), this
/// struct supports *return values* and *`RustCallStatus` handling* — both of which are
/// needed for the bidirectional VTable protocol.
///
/// The JS function is stored as a `napi_ref` (persistent reference with refcount = 1)
/// rather than a raw `napi_value`, because the VTable may outlive any single JS scope.
/// The reference prevents the garbage collector from reclaiming the function.
///
/// The `tsfn` field is `None` during construction and filled in by
/// [`build_vtable_struct`] before any concurrent access is possible. See the
/// "careful sequencing" discussion in that function's documentation.
pub struct VTableTrampolineUserdata {
    pub raw_env: napi::sys::napi_env,
    pub fn_ref: napi::sys::napi_ref,
    /// The declared arg types from the callback definition (not including RustCallStatus).
    pub arg_types: Vec<FfiTypeDesc>,
    /// The return type of this callback.
    pub ret_type: FfiTypeDesc,
    /// Whether the last C arg is a `&mut RustCallStatus`.
    pub has_rust_call_status: bool,
    /// Whether the return value is passed via an out-pointer argument (UniFFI 0.31+
    /// VTable convention). When true, the C function returns void and the return
    /// value is written to an extra pointer arg that sits between the declared args
    /// and the RustCallStatus pointer.
    pub out_return: bool,
    tsfn: Option<ThreadsafeFunction<VTableCallRequest, ErrorStrategy::Fatal>>,
    /// Resolved function pointers for `rustbuffer_from_bytes` and `rustbuffer_free`.
    /// Used to allocate RustBuffers from byte data and to free RustBuffer arguments
    /// received from C before converting to JS.
    pub rb_ops: RustBufferOps,
    /// All callback definitions, needed for wrapping `Callback`-typed arguments
    /// (C function pointers) as callable JS functions.
    pub callback_defs: HashMap<String, CallbackDef>,
    /// All struct definitions, needed for struct-by-value marshalling when
    /// wrapping C function pointers.
    pub struct_defs: HashMap<String, StructDef>,
}

// SAFETY: `raw_env` and `fn_ref` are only dereferenced on the main (JS) thread.
// The `tsfn` field is itself `Send` + `Sync`. The `rb_ops` contains plain C
// function pointers into a loaded shared library and are safe to call from any
// thread. The struct is leaked to a `'static` reference before any concurrent
// access occurs, so there are no data races.
unsafe impl Send for VTableTrampolineUserdata {}
unsafe impl Sync for VTableTrampolineUserdata {}

/// The top-level trampoline for VTable callbacks.
///
/// This is the function pointer stored in each libffi `Closure`. When Rust calls a
/// VTable function pointer, libffi invokes this trampoline with:
/// - `_cif`: the call interface (unused — we already know the types from `userdata`)
/// - `result`: a libffi-allocated buffer where we must write the return value
/// - `args`: an array of pointers to the C argument values
/// - `userdata`: our leaked [`VTableTrampolineUserdata`] containing type info and JS refs
///
/// The trampoline checks which thread it is on and dispatches accordingly.
///
/// # Safety
///
/// This function is `unsafe extern "C"` because it is called by libffi's closure
/// mechanism with raw C pointers. The caller (libffi) guarantees:
/// - `result` points to a buffer large enough for the declared return type.
/// - `args` contains exactly as many pointers as the CIF declared, each pointing
///   to a value of the corresponding type.
/// - `userdata` is the same reference we passed to `Closure::new`.
pub unsafe extern "C" fn vtable_trampoline_callback(
    _cif: &low::ffi_cif,
    result: &mut c_void,
    args: *const *const c_void,
    userdata: &VTableTrampolineUserdata,
) {
    // Zero-initialize the result buffer so that early returns (from error paths
    // in the main-thread or cross-thread trampolines) produce a deterministic
    // zero value rather than leaving uninitialized memory that the Rust caller
    // would interpret as a return value. We zero enough bytes to cover the
    // largest possible return type (RustBufferC at 24 bytes). For scalar returns
    // this zeros more than needed, which is harmless. For struct returns
    // (RustBuffer), this ensures all fields — including the `data` pointer —
    // are null/zero on error paths, preventing the caller from dereferencing
    // an uninitialized pointer.
    std::ptr::write_bytes(
        result as *mut c_void as *mut u8,
        0,
        std::mem::size_of::<RustBufferC>(),
    );

    if !is_main_thread() {
        // Cross-thread path: serialize args, dispatch to JS thread, wait for result
        vtable_trampoline_cross_thread(result, args, userdata);
        return;
    }

    // Main-thread path: call JS directly
    vtable_trampoline_main_thread(result, args, userdata);
}

/// Same-thread path: we are already on the JS main thread, so we can call the
/// JS function directly via N-API without any serialization overhead.
///
/// # Safety
///
/// Caller must ensure this is called on the main thread (the same thread that
/// owns `userdata.raw_env`). The `args` and `result` pointers come from libffi
/// and satisfy the same invariants as documented on [`vtable_trampoline_callback`].
unsafe fn vtable_trampoline_main_thread(
    result: &mut c_void,
    args: *const *const c_void,
    userdata: &VTableTrampolineUserdata,
) {
    let env = Env::from_raw(userdata.raw_env);

    // SAFETY: We are on the main thread. `fn_ref` was created with refcount=1
    // by `napi_create_reference` in `build_vtable_struct`, so the JS function
    // is alive and the reference is valid.
    let mut raw_fn: napi::sys::napi_value = std::ptr::null_mut();
    let status =
        napi::sys::napi_get_reference_value(userdata.raw_env, userdata.fn_ref, &mut raw_fn);
    if status != napi::sys::Status::napi_ok || raw_fn.is_null() {
        #[cfg(debug_assertions)]
        eprintln!("uniffi-runtime-napi: VTable trampoline failed to resolve JS function reference");
        return;
    }

    // SAFETY: `raw_fn` is a valid napi_value obtained from the reference above,
    // and we are on the correct env thread.
    let Ok(js_fn) = napi::JsFunction::from_raw(userdata.raw_env, raw_fn) else {
        #[cfg(debug_assertions)]
        eprintln!("uniffi-runtime-napi: VTable trampoline failed to reconstruct JsFunction");
        return;
    };

    let declared_count = userdata.arg_types.len();
    let mut js_args: Vec<napi::JsUnknown> = Vec::with_capacity(declared_count + 1);

    for (i, desc) in userdata.arg_types.iter().enumerate() {
        // SAFETY: libffi's CIF guarantees `args` has at least `declared_count`
        // entries, each pointing to a value whose type matches the CIF declaration.
        let arg_ptr = *args.add(i);
        let js_val = match desc {
            FfiTypeDesc::Callback(cb_name) => {
                // Wrap the C function pointer as a callable JS function.
                let fn_ptr = *(arg_ptr as *const *const c_void);
                let Some(cb_def) = userdata.callback_defs.get(cb_name) else {
                    #[cfg(debug_assertions)]
                    eprintln!("uniffi-runtime-napi: VTable trampoline has unknown callback def '{cb_name}'");
                    return;
                };
                match fn_pointer::create_fn_pointer_wrapper(
                    &env,
                    fn_ptr,
                    cb_def,
                    &userdata.struct_defs,
                    &userdata.rb_ops,
                ) {
                    Ok(f) => f.into_unknown(),
                    Err(_e) => {
                        #[cfg(debug_assertions)]
                        eprintln!("uniffi-runtime-napi: VTable trampoline failed to wrap fn pointer for '{cb_name}': {_e}");
                        return;
                    }
                }
            }
            _ => match c_arg_to_js(&env, desc, arg_ptr, userdata.rb_ops.free_ptr) {
                Ok(v) => v,
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("uniffi-runtime-napi: VTable trampoline failed to convert arg {i} ({desc:?}) to JS: {_e}");
                    return;
                }
            },
        };
        js_args.push(js_val);
    }

    // Track the next arg index after declared args.
    // When out_return is true, args[next_idx] is the out-return pointer.
    let mut next_idx = declared_count;

    // Capture the out-return pointer if this callback uses the out-pointer convention.
    let mut out_return_ptr: *mut c_void = std::ptr::null_mut();
    if userdata.out_return {
        // SAFETY: The CIF was built with an extra pointer arg at position `declared_count`.
        let out_ret_arg_ptr = *args.add(next_idx);
        // `out_ret_arg_ptr` is a pointer-to-pointer: libffi passes pointer args as
        // pointers to the actual pointer value on the caller's stack.
        out_return_ptr = *(out_ret_arg_ptr as *const *mut c_void);
        next_idx += 1;
    }

    // Extract the RustCallStatus pointer, if this callback uses one.
    //
    // The RustCallStatus is passed by the Rust caller as the final argument.
    // Because it is a *pointer* argument, libffi passes it as a pointer-to-pointer:
    // `args[next_idx]` points to a `*mut RustCallStatus` on the caller's stack.
    // We dereference once to get the inner pointer, then cast to our minimal
    // `RustCallStatusForVTable` which accesses only the `code` field.
    let mut status_ptr: *mut RustCallStatusForVTable = std::ptr::null_mut();
    if userdata.has_rust_call_status {
        // SAFETY: The CIF was built with the RustCallStatus pointer at this index.
        let rcs_arg_ptr = *args.add(next_idx);
        // SAFETY: `rcs_arg_ptr` points to a `*mut RustCallStatus`. Casting to
        // `*mut RustCallStatusForVTable` is layout-compatible because `code: i8`
        // is the first field of both structs.
        status_ptr = *(rcs_arg_ptr as *const *mut RustCallStatusForVTable);
    }

    // When out_return is true, the JS callback uses the UniffiResult protocol:
    // it returns { code, pointee?, errorBuf? } instead of accepting a status arg.
    if userdata.has_rust_call_status && !userdata.out_return {
        let code = if !status_ptr.is_null() {
            (*status_ptr).code as i32
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

    if userdata.out_return {
        // UniffiResult protocol: extract code, pointee, and errorBuf from the
        // returned JS object and write them to the C out-pointers.
        if let Ok(js_ret) = call_result {
            if let Ok(result_obj) = JsObject::from_raw(userdata.raw_env, js_ret.raw()) {
                write_uniffi_result_status(
                    &result_obj,
                    status_ptr,
                    userdata.raw_env,
                    userdata.rb_ops.from_bytes_ptr,
                );
                if !out_return_ptr.is_null() {
                    if let Ok(pointee) = result_obj.get_named_property::<napi::JsUnknown>("pointee")
                    {
                        write_return_value(
                            out_return_ptr,
                            &userdata.ret_type,
                            userdata.raw_env,
                            pointee,
                            userdata.rb_ops.from_bytes_ptr,
                            false,
                        );
                    }
                }
            }
        }
    } else {
        // Pass-by-reference status protocol: read back the mutated status object.
        if userdata.has_rust_call_status && !status_ptr.is_null() {
            if let Some(js_status_unknown) = js_args.last() {
                if let Ok(js_status_obj) =
                    JsObject::from_raw(userdata.raw_env, js_status_unknown.raw())
                {
                    if let Ok(code_val) = js_status_obj.get_named_property::<i32>("code") {
                        (*status_ptr).code = code_val as i8;
                    }
                }
            }
        }

        if let Ok(js_ret) = call_result {
            write_return_value(
                result as *mut c_void,
                &userdata.ret_type,
                userdata.raw_env,
                js_ret,
                userdata.rb_ops.from_bytes_ptr,
                true,
            );
        }
    }
}

/// Cross-thread path: the calling thread is *not* the JS main thread.
///
/// We cannot touch any N-API values here. Instead we:
/// 1. Read each C argument into a portable [`RawCallbackArg`] (pure memcpy, no JS).
/// 2. Bundle them into a [`VTableCallRequest`].
/// 3. If a return value or RustCallStatus writeback is needed (the common case),
///    create a `sync_channel(1)` rendezvous, send the request via the ThreadsafeFunction, and
///    block on `rx.recv()` until the JS thread fills in the response.
/// 4. If no return value is needed (e.g. the free callback), fire-and-forget.
///
/// # Safety
///
/// Same preconditions as [`vtable_trampoline_callback`]. Additionally, caller must
/// ensure this is called from a non-main thread (otherwise the blocking recv would
/// deadlock, since the JS thread is the one that must produce the response).
unsafe fn vtable_trampoline_cross_thread(
    result: &mut c_void,
    args: *const *const c_void,
    userdata: &VTableTrampolineUserdata,
) {
    let Some(tsfn) = &userdata.tsfn else {
        #[cfg(debug_assertions)]
        eprintln!("uniffi-runtime-napi: cross-thread VTable trampoline has no ThreadsafeFunction");
        return;
    };

    // Read C args into portable Rust values (no JS interaction needed).
    let declared_count = userdata.arg_types.len();
    let mut raw_args = Vec::with_capacity(declared_count);
    for (i, desc) in userdata.arg_types.iter().enumerate() {
        // SAFETY: libffi's CIF guarantees `args` has at least `declared_count`
        // entries, each pointing to a value of the corresponding type.
        let arg_ptr = *args.add(i);
        let Some(raw_arg) = read_raw_arg(desc, arg_ptr, userdata.rb_ops.free_ptr) else {
            #[cfg(debug_assertions)]
            eprintln!("uniffi-runtime-napi: cross-thread VTable trampoline failed to read arg {i} ({desc:?})");
            return;
        };
        raw_args.push(raw_arg);
    }

    // We need the blocking rendezvous whenever the caller expects something back:
    // either a return value or an updated RustCallStatus code.
    let needs_blocking =
        !matches!(userdata.ret_type, FfiTypeDesc::Void) || userdata.has_rust_call_status;

    // Track next arg index after declared args.
    let mut next_idx = declared_count;

    // Capture the out-return pointer if using the out-pointer convention.
    let mut out_return_ptr: *mut c_void = std::ptr::null_mut();
    if userdata.out_return {
        // SAFETY: CIF includes an extra pointer arg at this index.
        let out_ret_arg_ptr = *args.add(next_idx);
        out_return_ptr = *(out_ret_arg_ptr as *const *mut c_void);
        next_idx += 1;
    }

    // Read RustCallStatus code if present (same pointer-to-pointer pattern as
    // the main-thread path — see the SAFETY comments there).
    let mut status_ptr: *mut RustCallStatusForVTable = std::ptr::null_mut();
    let rcs_code = if userdata.has_rust_call_status {
        // SAFETY: CIF has the RustCallStatus pointer at this index.
        let rcs_arg_ptr = *args.add(next_idx);
        // SAFETY: pointer-to-pointer dereference; layout-compatible cast.
        status_ptr = *(rcs_arg_ptr as *const *mut RustCallStatusForVTable);
        if !status_ptr.is_null() {
            (*status_ptr).code
        } else {
            0
        }
    } else {
        0
    };

    if needs_blocking {
        // Blocking path: create a rendezvous channel. The capacity of 1 suffices
        // because there is exactly one producer (the JS thread handler) and one
        // consumer (this thread).
        let (tx, rx) = std::sync::mpsc::sync_channel(1);

        let request = VTableCallRequest {
            args: raw_args,
            rust_call_status_code: rcs_code,
            response_tx: Some(tx),
        };

        tsfn.call(request, ThreadsafeFunctionCallMode::Blocking);

        // Block until the JS thread processes the request and sends back the response.
        if let Ok(response) = rx.recv() {
            if let Some(ref raw_ret) = response.return_value {
                if userdata.out_return && !out_return_ptr.is_null() {
                    // Write the return value to the out-pointer at natural type width.
                    write_raw_return_value(
                        out_return_ptr,
                        &userdata.ret_type,
                        raw_ret,
                        userdata.rb_ops.from_bytes_ptr,
                        false,
                    );
                } else {
                    write_raw_return_value(
                        result as *mut c_void,
                        &userdata.ret_type,
                        raw_ret,
                        userdata.rb_ops.from_bytes_ptr,
                        true,
                    );
                }
            }
            if userdata.has_rust_call_status && !status_ptr.is_null() {
                // SAFETY: `status_ptr` is non-null (checked above) and points
                // to the Rust caller's stack-allocated RustCallStatus.
                (*status_ptr).code = response.rust_call_status_code;
            }
        }
    } else {
        // Non-blocking path: fire-and-forget (e.g. the VTable's free callback).
        let request = VTableCallRequest {
            args: raw_args,
            rust_call_status_code: rcs_code,
            response_tx: None,
        };

        tsfn.call(request, ThreadsafeFunctionCallMode::NonBlocking);
    }
}

/// Extract the `code` and `errorBuf` fields from a JS UniffiResult object and
/// write them into the C `RustCallStatus` via `status_ptr`.
///
/// # Safety
///
/// `status_ptr` must be null or point to a valid `RustCallStatus` on the caller's stack.
unsafe fn write_uniffi_result_status(
    result_obj: &JsObject,
    status_ptr: *mut RustCallStatusForVTable,
    raw_env: napi::sys::napi_env,
    rb_from_bytes_ptr: *const c_void,
) {
    if status_ptr.is_null() {
        return;
    }
    let Ok(code_val) = result_obj.get_named_property::<i32>("code") else {
        return;
    };
    (*status_ptr).code = code_val as i8;
    // Only propagate error details when the status indicates failure.
    if code_val != 0 {
        if let Ok(err_buf) = result_obj.get_named_property::<napi::JsUnknown>("errorBuf") {
            if let Ok(rb) =
                napi_utils::js_uint8array_to_rust_buffer(raw_env, err_buf, rb_from_bytes_ptr)
            {
                (*status_ptr).error_buf = rb;
            }
        }
    }
}

/// Write a JS return value into a destination buffer (same-thread path).
///
/// This function converts a `JsUnknown` returned by a JS callback into raw
/// bytes written to `dest`.
///
/// # The `widen_to_ffi_arg` parameter
///
/// When `true`, integer types smaller than 32 bits are widened to match the
/// libffi closure return convention:
/// - Signed types (`Int8`, `Int16`, `Int32`) are sign-extended to `ffi_sarg`.
/// - Unsigned types (`UInt8`, `UInt16`, `UInt32`) are zero-extended to `ffi_arg`.
///
/// When `false`, all integers are written at their natural width (`i8`, `u8`,
/// `i16`, etc.). Use this when writing to an out-pointer that points to a
/// concrete typed variable on the caller's stack.
///
/// `Float32`, `Float64`, `Int64`, `UInt64`, `Handle`, and `RustBuffer` are
/// always written at their natural width regardless of the flag.
///
/// # Safety
///
/// - `dest` must point to a writable buffer with sufficient size for the
///   declared return type (or `ffi_arg`/`ffi_sarg` when widening).
/// - `raw_env` must be the current thread's valid napi environment.
/// - `rb_from_bytes_ptr` (when non-null) must point to the library's
///   `rustbuffer_from_bytes` function with the signature [`RustBufferFromBytesFn`].
unsafe fn write_return_value(
    dest: *mut c_void,
    ret_type: &FfiTypeDesc,
    raw_env: napi::sys::napi_env,
    js_ret: napi::JsUnknown,
    rb_from_bytes_ptr: *const c_void,
    widen_to_ffi_arg: bool,
) {
    match ret_type {
        FfiTypeDesc::Void => {}
        FfiTypeDesc::Int8 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_sarg) = v as i8 as low::ffi_sarg;
                    } else {
                        *(dest as *mut i8) = v as i8;
                    }
                }
            }
        }
        FfiTypeDesc::UInt8 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_arg) = v as u8 as low::ffi_arg;
                    } else {
                        *(dest as *mut u8) = v as u8;
                    }
                }
            }
        }
        FfiTypeDesc::Int16 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_sarg) = v as i16 as low::ffi_sarg;
                    } else {
                        *(dest as *mut i16) = v as i16;
                    }
                }
            }
        }
        FfiTypeDesc::UInt16 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_arg) = v as u16 as low::ffi_arg;
                    } else {
                        *(dest as *mut u16) = v as u16;
                    }
                }
            }
        }
        FfiTypeDesc::Int32 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_sarg) = v as i32 as low::ffi_sarg;
                    } else {
                        *(dest as *mut i32) = v as i32;
                    }
                }
            }
        }
        FfiTypeDesc::UInt32 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    if widen_to_ffi_arg {
                        *(dest as *mut low::ffi_arg) = v as u32 as low::ffi_arg;
                    } else {
                        *(dest as *mut u32) = v as u32;
                    }
                }
            }
        }
        FfiTypeDesc::Int64 => {
            if let Ok(bigint) = napi::JsBigInt::from_raw(raw_env, js_ret.raw()) {
                if let Ok((v, _)) = bigint.get_i64() {
                    *(dest as *mut i64) = v;
                }
            }
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            if let Ok(bigint) = napi::JsBigInt::from_raw(raw_env, js_ret.raw()) {
                if let Ok((v, _)) = bigint.get_u64() {
                    *(dest as *mut u64) = v;
                }
            }
        }
        FfiTypeDesc::Float32 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    *(dest as *mut f32) = v as f32;
                }
            }
        }
        FfiTypeDesc::Float64 => {
            if let Ok(num) = napi::JsNumber::from_raw(raw_env, js_ret.raw()) {
                if let Ok(v) = num.get_double() {
                    *(dest as *mut f64) = v;
                }
            }
        }
        FfiTypeDesc::RustBuffer => {
            let Some((data, length)) = napi_utils::read_typedarray_data(raw_env, js_ret.raw())
            else {
                return;
            };
            if rb_from_bytes_ptr.is_null() {
                return;
            }
            if let Ok(rb) = napi_utils::rustbuffer_from_raw_bytes(data, length, rb_from_bytes_ptr) {
                *(dest as *mut RustBufferC) = rb;
            }
        }
        _ => {
            #[cfg(debug_assertions)]
            eprintln!("write_return_value: unsupported return type {ret_type:?}");
        }
    }
}

/// Write a [`RawCallbackArg`] return value into a destination buffer (cross-thread path).
///
/// This is the cross-thread counterpart to [`write_return_value`]. The JS thread has
/// already converted the JS return value into a [`RawCallbackArg`]; this function
/// writes it into the destination buffer on the *calling* thread.
///
/// # The `widen_to_ffi_arg` parameter
///
/// When `true`, integer types smaller than 32 bits are widened to match the
/// libffi closure return convention:
/// - Signed types (`Int8`, `Int16`, `Int32`) are sign-extended to `ffi_sarg`.
/// - Unsigned types (`UInt8`, `UInt16`, `UInt32`) are zero-extended to `ffi_arg`.
///
/// When `false`, all integers are written at their natural width (`i8`, `u8`,
/// `i16`, etc.). Use this when writing to an out-pointer that points to a
/// concrete typed variable on the caller's stack.
///
/// `Float32`, `Float64`, `Int64`, `UInt64`, `Handle`, and `RustBuffer` are
/// always written at their natural width regardless of the flag.
///
/// # Safety
///
/// - `dest` must point to a writable buffer with sufficient size.
/// - `rb_from_bytes_ptr` (when non-null) must be a valid `RustBufferFromBytesFn`.
///   `rustbuffer_from_bytes` is a pure C function, safe to call from any thread.
unsafe fn write_raw_return_value(
    dest: *mut c_void,
    ret_type: &FfiTypeDesc,
    raw_ret: &RawCallbackArg,
    rb_from_bytes_ptr: *const c_void,
    widen_to_ffi_arg: bool,
) {
    match (ret_type, raw_ret) {
        (FfiTypeDesc::Int8, RawCallbackArg::Int8(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_sarg) = *v as low::ffi_sarg;
            } else {
                *(dest as *mut i8) = *v;
            }
        }
        (FfiTypeDesc::UInt8, RawCallbackArg::UInt8(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_arg) = *v as low::ffi_arg;
            } else {
                *(dest as *mut u8) = *v;
            }
        }
        (FfiTypeDesc::Int16, RawCallbackArg::Int16(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_sarg) = *v as low::ffi_sarg;
            } else {
                *(dest as *mut i16) = *v;
            }
        }
        (FfiTypeDesc::UInt16, RawCallbackArg::UInt16(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_arg) = *v as low::ffi_arg;
            } else {
                *(dest as *mut u16) = *v;
            }
        }
        (FfiTypeDesc::Int32, RawCallbackArg::Int32(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_sarg) = *v as low::ffi_sarg;
            } else {
                *(dest as *mut i32) = *v;
            }
        }
        (FfiTypeDesc::UInt32, RawCallbackArg::UInt32(v)) => {
            if widen_to_ffi_arg {
                *(dest as *mut low::ffi_arg) = *v as low::ffi_arg;
            } else {
                *(dest as *mut u32) = *v;
            }
        }
        (FfiTypeDesc::Int64, RawCallbackArg::Int64(v)) => {
            *(dest as *mut i64) = *v;
        }
        (FfiTypeDesc::UInt64 | FfiTypeDesc::Handle, RawCallbackArg::UInt64(v)) => {
            *(dest as *mut u64) = *v;
        }
        (FfiTypeDesc::Float32, RawCallbackArg::Float32(v)) => {
            *(dest as *mut f32) = *v;
        }
        (FfiTypeDesc::Float64, RawCallbackArg::Float64(v)) => {
            *(dest as *mut f64) = *v;
        }
        (FfiTypeDesc::RustBuffer, RawCallbackArg::RustBuffer(data)) => {
            if rb_from_bytes_ptr.is_null() {
                return;
            }
            if let Ok(rb) =
                napi_utils::rustbuffer_from_raw_bytes(data.as_ptr(), data.len(), rb_from_bytes_ptr)
            {
                *(dest as *mut RustBufferC) = rb;
            }
        }
        _ => {
            #[cfg(debug_assertions)]
            eprintln!("write_raw_return_value: unsupported type pair {ret_type:?} / {raw_ret:?}");
        }
    }
}

/// Handler that runs on the JS main thread when a [`VTableCallRequest`] arrives via ThreadsafeFunction.
///
/// This is the "receiving end" of the cross-thread dispatch. It:
/// 1. Resolves the persistent JS function reference.
/// 2. Converts each [`RawCallbackArg`] back into a `JsUnknown`.
/// 3. Calls the JS function.
/// 4. Converts the JS return value into a [`RawCallbackArg`].
/// 5. Sends a [`VTableCallResponse`] back through the rendezvous channel,
///    unblocking the calling thread.
///
/// If any step fails, `send_default` sends a default response so the calling
/// thread does not deadlock.
fn vtable_tsfn_handler(env: &Env, userdata: &VTableTrampolineUserdata, request: VTableCallRequest) {
    // Helper: send a default (empty) response so the calling thread never hangs.
    let send_default = |req: &VTableCallRequest| {
        if let Some(ref tx) = req.response_tx {
            let _ = tx.send(VTableCallResponse {
                return_value: None,
                rust_call_status_code: req.rust_call_status_code,
            });
        }
    };

    // SAFETY: We are on the main thread (guaranteed by ThreadsafeFunction dispatch).
    // `fn_ref` was created with refcount=1 by `napi_create_reference`, so the JS
    // function is alive and the reference is valid.
    let mut raw_fn: napi::sys::napi_value = std::ptr::null_mut();
    let status = unsafe {
        napi::sys::napi_get_reference_value(userdata.raw_env, userdata.fn_ref, &mut raw_fn)
    };
    if status != napi::sys::Status::napi_ok || raw_fn.is_null() {
        send_default(&request);
        return;
    }

    // SAFETY: `raw_fn` is a valid napi_value obtained from the reference above,
    // and we are on the correct env thread.
    let js_fn = match unsafe { napi::JsFunction::from_raw(userdata.raw_env, raw_fn) } {
        Ok(f) => f,
        Err(_) => {
            send_default(&request);
            return;
        }
    };

    let mut js_args: Vec<napi::JsUnknown> = Vec::with_capacity(request.args.len() + 1);
    for (i, raw_arg) in request.args.iter().enumerate() {
        // For Callback-typed args, the cross-thread path transported the raw pointer
        // as RawCallbackArg::Pointer. We now wrap it as a callable JS function.
        let js_val = if let Some(FfiTypeDesc::Callback(cb_name)) = userdata.arg_types.get(i) {
            if let RawCallbackArg::Pointer(ptr_val) = raw_arg {
                let fn_ptr = *ptr_val as *const c_void;
                let cb_def = match userdata.callback_defs.get(cb_name) {
                    Some(d) => d,
                    None => {
                        send_default(&request);
                        return;
                    }
                };
                match fn_pointer::create_fn_pointer_wrapper(
                    env,
                    fn_ptr,
                    cb_def,
                    &userdata.struct_defs,
                    &userdata.rb_ops,
                ) {
                    Ok(f) => f.into_unknown(),
                    Err(_) => {
                        send_default(&request);
                        return;
                    }
                }
            } else {
                match raw_arg_to_js(env, raw_arg) {
                    Ok(v) => v,
                    Err(_) => {
                        send_default(&request);
                        return;
                    }
                }
            }
        } else {
            match raw_arg_to_js(env, raw_arg) {
                Ok(v) => v,
                Err(_) => {
                    send_default(&request);
                    return;
                }
            }
        };
        js_args.push(js_val);
    }

    // When out_return is true, the JS callback uses the UniffiResult protocol.
    if userdata.has_rust_call_status && !userdata.out_return {
        let mut js_status = match env.create_object() {
            Ok(o) => o,
            Err(_) => {
                send_default(&request);
                return;
            }
        };
        let code_val = match env.create_int32(request.rust_call_status_code as i32) {
            Ok(v) => v,
            Err(_) => {
                send_default(&request);
                return;
            }
        };
        if js_status.set_named_property("code", code_val).is_err() {
            send_default(&request);
            return;
        }
        js_args.push(js_status.into_unknown());
    }

    let call_result = js_fn.call(None, &js_args);

    if request.response_tx.is_none() {
        return;
    }

    if userdata.out_return {
        // UniffiResult protocol: extract code and pointee from the returned object.
        let (rcs_code, return_value) = match call_result {
            Ok(js_ret) => {
                if let Ok(result_obj) =
                    unsafe { JsObject::from_raw(userdata.raw_env, js_ret.raw()) }
                {
                    let code = result_obj
                        .get_named_property::<i32>("code")
                        .map(|c| c as i8)
                        .unwrap_or(request.rust_call_status_code);
                    let pointee = if matches!(userdata.ret_type, FfiTypeDesc::Void) {
                        None
                    } else {
                        result_obj
                            .get_named_property::<napi::JsUnknown>("pointee")
                            .ok()
                            .and_then(|v| js_return_to_raw(env, &userdata.ret_type, v))
                    };
                    (code, pointee)
                } else {
                    (request.rust_call_status_code, None)
                }
            }
            Err(_) => (request.rust_call_status_code, None),
        };

        if let Some(tx) = request.response_tx {
            let _ = tx.send(VTableCallResponse {
                return_value,
                rust_call_status_code: rcs_code,
            });
        }
    } else {
        // Pass-by-reference status protocol.
        let rcs_code = if userdata.has_rust_call_status {
            if let Some(js_status_unknown) = js_args.last() {
                if let Ok(js_status_obj) =
                    unsafe { JsObject::from_raw(userdata.raw_env, js_status_unknown.raw()) }
                {
                    js_status_obj
                        .get_named_property::<i32>("code")
                        .map(|c| c as i8)
                        .unwrap_or(request.rust_call_status_code)
                } else {
                    request.rust_call_status_code
                }
            } else {
                request.rust_call_status_code
            }
        } else {
            0
        };

        let return_value = match call_result {
            Ok(js_ret) => {
                if matches!(userdata.ret_type, FfiTypeDesc::Void) {
                    None
                } else {
                    js_return_to_raw(env, &userdata.ret_type, js_ret)
                }
            }
            Err(_) => None,
        };

        if let Some(tx) = request.response_tx {
            let _ = tx.send(VTableCallResponse {
                return_value,
                rust_call_status_code: rcs_code,
            });
        }
    }
}

/// `#[repr(C)]` projection of `RustCallStatus`, layout-compatible with the real struct.
///
/// The full `RustCallStatus` struct (defined in the UniFFI runtime) looks like:
///
/// ```text
/// #[repr(C)]
/// struct RustCallStatus {
///     code: i8,
///     error_buf: RustBuffer,
/// }
/// ```
///
/// Because both structs are `#[repr(C)]` and the fields match, a `*mut RustCallStatus`
/// can be safely cast to `*mut RustCallStatusForVTable`.
#[repr(C)]
struct RustCallStatusForVTable {
    code: i8,
    error_buf: RustBufferC,
}

/// Build a C-compatible VTable struct from a JS object implementing a UniFFI trait.
///
/// For each field in the struct definition that is a `Callback(name)`:
/// 1. Look up the callback definition to learn the parameter and return types.
/// 2. Extract the corresponding JS function from the JS object.
/// 3. Create a persistent `napi_ref` (refcount = 1) to prevent GC of the function.
/// 4. Build a libffi `Closure` backed by [`vtable_trampoline_callback`].
/// 5. Store the closure's code pointer in the VTable data array.
///
/// ## Careful sequencing of `VTableTrampolineUserdata` initialization
///
/// The userdata struct is heap-allocated, then leaked via `Box::into_raw` *before*
/// the `ThreadsafeFunction` is created. This is necessary because the ThreadsafeFunction closure
/// captures the userdata address. The sequence is:
///
/// 1. `Box::into_raw(userdata)` — gives us a stable raw pointer.
/// 2. Create the ThreadsafeFunction, whose closure captures `userdata_ptr as usize`.
/// 3. Write the ThreadsafeFunction into `(*userdata_ptr).tsfn` via raw pointer mutation.
/// 4. *Only then* create `&'static VTableTrampolineUserdata` from the raw pointer.
///
/// After step 4, no further mutation occurs. The `&'static` reference is passed to
/// `Closure::new`, which requires `'static` because the closure may be called at
/// any time for the lifetime of the process.
///
/// ## Intentional leaks
///
/// The `Closure`, the `VTableTrampolineUserdata`, and the `Vec<*const c_void>`
/// backing the struct are all leaked (`mem::forget` / `Box::into_raw`). This is
/// *not* a memory bug — these allocations must live for the process lifetime because
/// the Rust library holds raw function pointers into them and may call them at any
/// time from any thread. There is no VTable deregistration protocol in UniFFI.
///
/// Returns a raw pointer to the first element of the leaked `Vec`, which is the
/// C-compatible VTable struct pointer expected by the Rust library.
pub fn build_vtable_struct(
    env: &Env,
    struct_def: &StructDef,
    js_obj: &JsObject,
    callback_defs: &HashMap<String, CallbackDef>,
    struct_defs: &HashMap<String, StructDef>,
    rb_ops: &RustBufferOps,
) -> Result<*const c_void> {
    let field_count = struct_def.fields.len();
    let mut vtable_data: Vec<*const c_void> = Vec::with_capacity(field_count);

    for field in &struct_def.fields {
        match &field.field_type {
            FfiTypeDesc::Callback(cb_name) => {
                let cb_def = callback_defs.get(cb_name).ok_or_else(|| {
                    napi::Error::from_reason(format!(
                        "Unknown callback '{cb_name}' for struct field '{}'",
                        field.name
                    ))
                })?;

                // Get the JS function from the object.
                let js_fn_val: JsUnknown = js_obj.get_named_property(&field.name)?;
                // SAFETY: Extracting the raw napi_value from a live JsUnknown on the
                // current env thread.
                let raw_fn_val = unsafe { js_fn_val.raw() };

                // Create a persistent reference (refcount = 1) to prevent GC.
                // This reference is intentionally never deleted — see "Lifetime
                // management" in the module docs.
                let mut fn_ref: napi::sys::napi_ref = std::ptr::null_mut();
                // SAFETY: `env.raw()` is the current valid env, `raw_fn_val` is a
                // live napi_value. The initial refcount of 1 keeps the JS function
                // alive for the process lifetime.
                let ref_status = unsafe {
                    napi::sys::napi_create_reference(env.raw(), raw_fn_val, 1, &mut fn_ref)
                };
                if ref_status != napi::sys::Status::napi_ok {
                    return Err(napi::Error::from_reason(format!(
                        "Failed to create reference for VTable field '{}'",
                        field.name
                    )));
                }

                // Build CIF for this callback:
                // declared args + optional out-return pointer + optional RustCallStatus pointer
                let mut cif_arg_types: Vec<Type> = cb_def
                    .args
                    .iter()
                    .map(|a| ffi_type_for(a, struct_defs))
                    .collect::<napi::Result<Vec<_>>>()?;
                if cb_def.out_return {
                    // Out-return pointer: an extra pointer arg before RustCallStatus
                    cif_arg_types.push(Type::pointer());
                }
                if cb_def.has_rust_call_status {
                    cif_arg_types.push(Type::pointer());
                }
                // When out_return is true, the C function returns void; the return
                // value is written through the out-pointer instead.
                let cif_ret_type = if cb_def.out_return {
                    Type::void()
                } else {
                    ffi_type_for(&cb_def.ret, struct_defs)?
                };
                let cif = Cif::new(cif_arg_types, cif_ret_type);

                // Create userdata
                let userdata = Box::new(VTableTrampolineUserdata {
                    raw_env: env.raw(),
                    fn_ref,
                    arg_types: cb_def.args.clone(),
                    ret_type: cb_def.ret.clone(),
                    has_rust_call_status: cb_def.has_rust_call_status,
                    out_return: cb_def.out_return,
                    tsfn: None, // Will be set below.
                    rb_ops: *rb_ops,
                    callback_defs: callback_defs.clone(),
                    struct_defs: struct_defs.clone(),
                });

                // Leak userdata to a raw pointer for a stable address.
                // Do NOT create &'static ref yet — we still need to mutate tsfn.
                // This is step 1 of the sequencing described in the function docs.
                let userdata_ptr = Box::into_raw(userdata);

                // Create a no-op JS function for the ThreadsafeFunction base. The ThreadsafeFunction auto-calls its base
                // function with the callback's returned Vec<JsUnknown>. By using a no-op,
                // the auto-call is harmless — the real JS function is called manually in
                // vtable_tsfn_handler via fn_ref.
                let noop_fn =
                    env.create_function_from_closure("vtable_tsfn_noop", |_ctx| Ok(()))?;

                // Step 2: create the ThreadsafeFunction. The closure captures the userdata address
                // as a `usize` rather than a raw pointer, because raw pointers are
                // not `Send` and the closure must be `Send` for `ThreadsafeFunction`.
                let tsfn: ThreadsafeFunction<VTableCallRequest, ErrorStrategy::Fatal> = {
                    let ud_addr = userdata_ptr as usize;
                    noop_fn.create_threadsafe_function(
                        0,
                        move |ctx: napi::threadsafe_function::ThreadSafeCallContext<
                            VTableCallRequest,
                        >| {
                            // SAFETY: `ud_addr` was obtained from `Box::into_raw` above,
                            // so the allocation is valid and leaked (lives forever).
                            // `ud_addr` was set before any concurrent access is possible
                            // (the ThreadsafeFunction hasn't been called yet — it is created here and
                            // only becomes callable after `tsfn.call()` is invoked by a
                            // trampoline, which cannot happen until `build_vtable_struct`
                            // returns the VTable pointer to the Rust library).
                            let ud = unsafe { &*(ud_addr as *const VTableTrampolineUserdata) };
                            vtable_tsfn_handler(&ctx.env, ud, ctx.value);
                            Ok(Vec::<napi::JsUnknown>::new())
                        },
                    )?
                };

                // Unref so it doesn't keep the event loop alive.
                let mut tsfn = tsfn;
                tsfn.unref(env)?;

                // Step 3: store the ThreadsafeFunction into the userdata via raw pointer mutation.
                // SAFETY: `userdata_ptr` is a valid, uniquely-owned heap allocation
                // (no other references exist yet). We are still on the main thread
                // and no concurrent access is possible.
                unsafe {
                    (*userdata_ptr).tsfn = Some(tsfn);
                }

                // Step 4: all mutation is complete. NOW it is safe to create the
                // `&'static` reference that `Closure::new` requires.
                // SAFETY: The allocation is leaked (lives forever) and fully
                // initialized. No further mutation occurs after this point.
                let userdata_ref: &'static VTableTrampolineUserdata = unsafe { &*userdata_ptr };

                // Create closure
                let closure = Closure::new(cif, vtable_trampoline_callback, userdata_ref);

                // Extract the C function pointer from the closure.
                let fn_ptr: *const c_void = *closure.code_ptr() as *const std::ffi::c_void;

                // Intentionally leak the closure. The function pointer we extracted
                // above points into the closure's executable trampoline memory. If
                // the closure were dropped, the function pointer would dangle.
                std::mem::forget(closure);

                vtable_data.push(fn_ptr);
            }
            _ => {
                return Err(napi::Error::from_reason(format!(
                    "Unsupported struct field type for '{}': {:?}. \
                     Only Callback fields are supported in VTable structs.",
                    field.name, field.field_type
                )));
            }
        }
    }

    // Intentionally leak the Vec so the VTable struct memory persists for the
    // process lifetime. The Rust library holds a raw pointer to this array and
    // reads function pointers from it on every VTable method call.
    let ptr = vtable_data.as_ptr() as *const c_void;
    std::mem::forget(vtable_data);

    Ok(ptr)
}
