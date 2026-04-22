/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # Callback trampolines: routing C function calls back into JavaScript
//!
//! ## The Callback Problem
//!
//! When a UniFFI-generated Rust library needs to call back into JavaScript, we
//! face a fundamental threading constraint: napi values (handles into the V8
//! heap) can only be created and accessed on the main Node.js thread. But the
//! Rust library may invoke our callback from *any* thread — a background
//! thread pool, a networking thread, etc. This module solves that problem with
//! a **two-path trampoline architecture**.
//!
//! ## The Trampoline
//!
//! We use **libffi closures** to mint C-callable function pointers whose actual
//! implementation is our Rust function [`trampoline_callback`]. When C code
//! calls one of these function pointers, libffi invokes the trampoline, which
//! checks [`is_main_thread()`]:
//!
//! - **Same-thread path** ([`trampoline_main_thread`]): reconstruct a napi
//!   [`Env`] from the raw pointer stashed in [`TrampolineUserdata`], convert
//!   each C argument directly into a JS value via [`c_arg_to_js`], and call
//!   the JS function immediately. This is the fast path — no copies, no
//!   serialisation, no event-loop round-trip.
//!
//! - **Cross-thread path** ([`trampoline_cross_thread`]): we cannot touch
//!   napi at all. Instead, each C argument is read into a [`RawCallbackArg`]
//!   (a safe, `Send + Sync` enum), and the resulting `Vec<RawCallbackArg>` is
//!   dispatched to the main thread via a [`ThreadsafeFunction`]. On arrival,
//!   [`raw_arg_to_js`] converts each element into a JS value.
//!
//! ## `RawCallbackArg` — the cross-thread transport type
//!
//! [`RawCallbackArg`] is essentially a tagged union of every scalar FFI type
//! plus `Vec<u8>` for `RustBuffer` data. The two-phase conversion
//! (C pointer → `RawCallbackArg` → JS value) adds an intermediate copy for
//! buffer data, but this is **unavoidable**: the C pointer may become invalid
//! the instant the calling thread resumes after the trampoline returns, so we
//! must capture the data before yielding control.
//!
//! ## Ownership conventions for `RustBuffer` arguments
//!
//! When the Rust library passes a `RustBuffer` into a callback, ownership
//! transfers to the callback — we are responsible for freeing it.
//!
//! - [`read_raw_arg`] (cross-thread): copies the buffer data into a `Vec`,
//!   then frees the original `RustBuffer` via `free_rustbuffer`. The copy is
//!   safe to send across threads; the original must be freed on the calling
//!   thread before we return.
//!
//! - [`c_arg_to_js`] (same-thread): copies the buffer data directly into a
//!   JS `Uint8Array` (avoiding the intermediate `Vec`), then frees the
//!   original. The JS garbage collector owns the copy from that point on.

use std::collections::HashMap;
use std::ffi::c_void;

use libffi::low;
use libffi::middle::{Cif, Type};
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Env, NapiRaw, NapiValue};

use crate::cif::ffi_type_for;
use crate::ffi_c_types::RustBufferC;
use crate::ffi_type::FfiTypeDesc;
use crate::is_main_thread;
use crate::napi_utils;
use crate::structs::StructDef;

/// A parsed description of a single callback's signature.
///
/// Built from the JS-side `callbacks` map, where each entry describes the
/// argument types, return type, and whether the callback includes an
/// out-parameter `RustCallStatus` (used by UniFFI for error propagation).
/// This struct drives both CIF construction (via [`build_callback_cif`]) and
/// argument marshalling in the trampoline.
#[derive(Debug, Clone)]
pub struct CallbackDef {
    /// The FFI types of the callback's positional arguments.
    pub args: Vec<FfiTypeDesc>,
    /// The FFI type of the callback's return value.
    pub ret: FfiTypeDesc,
    /// Whether the callback signature includes a trailing `RustCallStatus`
    /// out-pointer, used by UniFFI's error-propagation convention.
    pub has_rust_call_status: bool,
    /// Whether the return value is passed via an out-pointer argument rather
    /// than the C return value. When true, the C function returns `void` and
    /// the last argument before `RustCallStatus` is a pointer to the return
    /// value buffer. This is the UniFFI 0.31+ convention for VTable callbacks.
    pub out_return: bool,
}

/// A thread-safe snapshot of a single C argument value.
///
/// This enum is the **key type enabling cross-thread transport**. When a
/// callback is invoked off the main thread, we cannot create napi values, so
/// we read each C argument into one of these variants instead. The resulting
/// `Vec<RawCallbackArg>` is `Send + Sync` and can be safely shipped to the
/// main thread via [`ThreadsafeFunction`], where [`raw_arg_to_js`] converts
/// each element into its JS equivalent.
///
/// For scalar types the cost is negligible (a `Copy` of a few bytes). For
/// `RustBuffer` data the cost is a heap allocation and `memcpy` — unavoidable,
/// because the original buffer's lifetime ends when the trampoline returns.
#[derive(Clone, Debug)]
pub enum RawCallbackArg {
    UInt8(u8),
    Int8(i8),
    UInt16(u16),
    Int16(i16),
    UInt32(u32),
    Int32(i32),
    UInt64(u64),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    /// Buffer data copied out of a `RustBuffer` for cross-thread transport.
    /// The original `RustBuffer` is freed immediately after the copy.
    RustBuffer(Vec<u8>),
    /// A raw C pointer (function pointer or opaque reference) transported as usize.
    Pointer(usize),
    /// Pre-marshalled struct bytes for cross-thread transport.
    /// Used when a callback returns a by-value struct (e.g. ForeignFutureDroppedCallbackStruct).
    StructBytes(Vec<u8>),
}

/// Per-closure state passed to the libffi trampoline as the `userdata` pointer.
///
/// An instance of this struct is created when a callback closure is minted and
/// leaked via `Box::into_raw` so that it lives for the lifetime of the
/// closure (effectively `'static` — closures are never unregistered). The
/// trampoline receives it as `&TrampolineUserdata` on every invocation.
///
/// The struct carries everything the trampoline needs for both dispatch paths:
/// - `raw_env` / `fn_ref`: raw napi env handle and GC-preventing reference for the same-thread path.
/// - `tsfn`: an optional [`ThreadsafeFunction`] for the cross-thread path.
/// - `arg_types`: the FFI type descriptors that tell us how to interpret each
///   raw pointer in the libffi `args` array.
/// - `rb_free_ptr`: a function pointer to `rustbuffer_free`, needed to release
///   `RustBuffer` arguments after we have copied their data.
pub struct TrampolineUserdata {
    /// Raw napi environment handle. Only valid on the main thread.
    pub raw_env: napi::sys::napi_env,
    /// GC-preventing reference to the JS callback function. Only valid on the main thread.
    pub fn_ref: napi::sys::napi_ref,
    /// FFI type descriptors for each positional argument, used to interpret
    /// the raw pointers in the libffi `args` array.
    pub arg_types: Vec<FfiTypeDesc>,
    /// Thread-safe function for dispatching to the main thread. `None` if
    /// cross-thread dispatch is not needed (e.g., if the callback is known
    /// to be invoked only on the main thread).
    pub tsfn: Option<ThreadsafeFunction<Vec<RawCallbackArg>, ErrorStrategy::Fatal>>,
    /// Pointer to the UniFFI-generated `rustbuffer_free` C function. Needed to
    /// free `RustBuffer` arguments whose ownership transfers to this callback.
    /// We do not need `rustbuffer_from_bytes` because simple callbacks never
    /// return `RustBuffer` values.
    pub rb_free_ptr: *const c_void,
}

// SAFETY: `TrampolineUserdata` is shared across threads, which requires both
// `Send` and `Sync`. This is sound because:
// - `raw_env` and `raw_fn` are only dereferenced on the main thread (the
//   same-thread path checks `is_main_thread()` before touching them).
// - `tsfn` (`ThreadsafeFunction`) is explicitly designed for cross-thread use.
// - `arg_types` is an immutable `Vec` after construction — no mutation, no races.
// - `rb_free_ptr` is a plain function pointer to a C function that is safe to
//   call from any thread.
unsafe impl Send for TrampolineUserdata {}
unsafe impl Sync for TrampolineUserdata {}

/// The top-level trampoline invoked by libffi when C code calls the closure's
/// function pointer.
///
/// This is the entry point for *every* callback invocation. It inspects the
/// current thread and delegates to the appropriate path:
/// - [`trampoline_main_thread`] for direct JS calls (fast path), or
/// - [`trampoline_cross_thread`] for serialised dispatch via ThreadsafeFunction.
///
/// # Safety
///
/// This function is called from C through a libffi closure. The caller must
/// uphold the following contracts (all guaranteed by libffi's closure
/// machinery):
///
/// - `args` is a valid pointer to an array of `*const c_void`, with exactly
///   as many entries as the CIF declared. Each entry points to a value whose
///   type and alignment match the corresponding CIF argument type.
/// - `userdata` was originally created via `Box::into_raw` and has not been
///   freed. It is treated as `&'static TrampolineUserdata`.
/// - `_cif` and `_result` are valid libffi-managed pointers for the duration
///   of this call.
pub unsafe extern "C" fn trampoline_callback(
    _cif: &low::ffi_cif,
    _result: &mut c_void,
    args: *const *const c_void,
    userdata: &TrampolineUserdata,
) {
    if crate::is_shutting_down() {
        return;
    }

    if is_main_thread() {
        // Same-thread path: call JS function directly.
        trampoline_main_thread(_cif, _result, args, userdata);
    } else {
        // Cross-thread path: serialize args and dispatch via ThreadsafeFunction.
        trampoline_cross_thread(args, userdata);
    }
}

/// Same-thread fast path: convert C arguments directly to JS values and call
/// the JS function synchronously.
///
/// # Safety
///
/// Must only be called on the main Node.js thread (caller checks
/// `is_main_thread()`). The same preconditions as [`trampoline_callback`]
/// apply to `args` and `userdata`. Additionally:
///
/// - `userdata.raw_env` is a valid `napi_env` because we are on the main
///   thread where it was originally captured.
/// - `userdata.raw_fn` is a valid `napi_value` referencing a JS function,
///   prevented from garbage collection by an prevent ref held elsewhere.
unsafe fn trampoline_main_thread(
    _cif: &low::ffi_cif,
    _result: &mut c_void,
    args: *const *const c_void,
    userdata: &TrampolineUserdata,
) {
    // SAFETY: We are on the main thread, so raw_env is valid.
    let env = Env::from_raw(userdata.raw_env);

    // SAFETY: We are on the main thread. `fn_ref` was created with refcount=1
    // by `napi_create_reference`, so the JS function is alive and the reference is valid.
    let mut raw_fn: napi::sys::napi_value = std::ptr::null_mut();
    let status =
        napi::sys::napi_get_reference_value(userdata.raw_env, userdata.fn_ref, &mut raw_fn);
    if status != napi::sys::Status::napi_ok || raw_fn.is_null() {
        #[cfg(debug_assertions)]
        eprintln!(
            "uniffi-runtime-napi: callback trampoline failed to resolve JS function reference"
        );
        return;
    }

    // SAFETY: `raw_fn` is a valid napi_value obtained from the reference above,
    // and we are on the correct env thread.
    let Ok(js_fn) = napi::JsFunction::from_raw(userdata.raw_env, raw_fn) else {
        #[cfg(debug_assertions)]
        eprintln!("uniffi-runtime-napi: callback trampoline failed to reconstruct JsFunction");
        return;
    };

    let arg_count = userdata.arg_types.len();

    let mut js_args: Vec<napi::JsUnknown> = Vec::with_capacity(arg_count);
    for (i, desc) in userdata.arg_types.iter().enumerate() {
        // SAFETY: libffi guarantees `args` has at least `arg_count` entries,
        // so `args.add(i)` is in bounds. The double dereference yields a
        // pointer to the actual C value, whose type matches `desc` because
        // the CIF was built from the same `arg_types`.
        let arg_ptr = *args.add(i);
        let Ok(js_val) = c_arg_to_js(&env, desc, arg_ptr, userdata.rb_free_ptr) else {
            #[cfg(debug_assertions)]
            eprintln!("uniffi-runtime-napi: callback trampoline failed to convert arg {i} ({desc:?}) to JS");
            return;
        };
        js_args.push(js_val);
    }

    let _result = js_fn.call(None, &js_args);
}

/// Cross-thread path: snapshot every C argument into a [`RawCallbackArg`] and
/// dispatch the batch to the main thread via [`ThreadsafeFunction`].
///
/// This function does *not* touch any napi APIs — it only reads raw memory and
/// enqueues a message. The actual JS call happens later, on the main thread,
/// when the event loop drains the ThreadsafeFunction queue.
///
/// # Safety
///
/// Same preconditions as [`trampoline_callback`] for `args` and `userdata`.
/// Must be called from a non-main thread (caller checks `!is_main_thread()`),
/// though calling it on the main thread would be safe — just wasteful.
unsafe fn trampoline_cross_thread(args: *const *const c_void, userdata: &TrampolineUserdata) {
    let Some(tsfn) = &userdata.tsfn else {
        #[cfg(debug_assertions)]
        eprintln!(
            "uniffi-runtime-napi: cross-thread callback has no ThreadsafeFunction, dropping call"
        );
        return;
    };

    let mut raw_args = Vec::with_capacity(userdata.arg_types.len());
    for (i, desc) in userdata.arg_types.iter().enumerate() {
        // SAFETY: libffi guarantees `args` has exactly `arg_types.len()`
        // entries. Each `*args.add(i)` yields a pointer to a value whose
        // type matches `desc`.
        let arg_ptr = *args.add(i);
        let Some(raw_arg) = read_raw_arg(desc, arg_ptr, userdata.rb_free_ptr) else {
            #[cfg(debug_assertions)]
            eprintln!(
                "uniffi-runtime-napi: cross-thread callback failed to read arg {i} ({desc:?})"
            );
            return;
        };
        raw_args.push(raw_arg);
    }

    // Dispatch to the main thread. `NonBlocking` means we enqueue without
    // waiting for the JS side to process the call — the Rust caller resumes
    // immediately. This is appropriate for void-returning callbacks.
    tsfn.call(raw_args, ThreadsafeFunctionCallMode::NonBlocking);
}

/// Read a single C argument from a raw pointer and return it as a
/// [`RawCallbackArg`], suitable for cross-thread transport.
///
/// For scalar types this is a trivial pointer dereference and copy. For
/// `RustBuffer` arguments, this function:
///   1. Copies the buffer data into a new `Vec<u8>`,
///   2. Frees the original `RustBuffer` via `free_rustbuffer`.
///
/// Step 2 is critical: the Rust library allocated the `RustBuffer` and
/// transferred ownership to this callback. We must free it before returning
/// control to the caller, because the caller may deallocate the stack frame
/// that holds the `RustBufferC` struct.
///
/// # Safety
///
/// - `arg_ptr` must point to a value whose type matches `desc`. This is
///   guaranteed by libffi: the CIF was built from the same [`FfiTypeDesc`]
///   list, so the pointer's type and alignment are correct.
/// - For `RustBuffer`: `arg_ptr` must point to a valid `RustBufferC` whose
///   `data` field (if non-null) points to `len` readable bytes. After this
///   call, the original buffer is freed and must not be used again.
/// - `rb_free_ptr` must be a valid pointer to the UniFFI-generated
///   `rustbuffer_free` function.
pub unsafe fn read_raw_arg(
    desc: &FfiTypeDesc,
    arg_ptr: *const c_void,
    rb_free_ptr: *const c_void,
) -> Option<RawCallbackArg> {
    match desc {
        // SAFETY (all scalar arms): The `FfiTypeDesc` was used to build the
        // CIF, so libffi guarantees `arg_ptr` points to a value of exactly
        // the cast-to type with correct alignment. The dereference reads a
        // `Copy` type and does not take ownership.
        FfiTypeDesc::UInt8 => Some(RawCallbackArg::UInt8(*(arg_ptr as *const u8))),
        FfiTypeDesc::Int8 => Some(RawCallbackArg::Int8(*(arg_ptr as *const i8))),
        FfiTypeDesc::UInt16 => Some(RawCallbackArg::UInt16(*(arg_ptr as *const u16))),
        FfiTypeDesc::Int16 => Some(RawCallbackArg::Int16(*(arg_ptr as *const i16))),
        FfiTypeDesc::UInt32 => Some(RawCallbackArg::UInt32(*(arg_ptr as *const u32))),
        FfiTypeDesc::Int32 => Some(RawCallbackArg::Int32(*(arg_ptr as *const i32))),
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            Some(RawCallbackArg::UInt64(*(arg_ptr as *const u64)))
        }
        FfiTypeDesc::Int64 => Some(RawCallbackArg::Int64(*(arg_ptr as *const i64))),
        FfiTypeDesc::Float32 => Some(RawCallbackArg::Float32(*(arg_ptr as *const f32))),
        FfiTypeDesc::Float64 => Some(RawCallbackArg::Float64(*(arg_ptr as *const f64))),
        FfiTypeDesc::RustBuffer => {
            // SAFETY: `arg_ptr` points to a `RustBufferC` (guaranteed by the CIF).
            let rb = *(arg_ptr as *const RustBufferC);
            let Ok(len) = usize::try_from(rb.len) else {
                // rb.len exceeds addressable memory — free the buffer and bail.
                napi_utils::free_rustbuffer(rb, rb_free_ptr);
                return None;
            };

            // Copy the buffer data into an owned Vec for safe cross-thread transport.
            // SAFETY: `rb.data` points to at least `len` readable bytes
            // (contract of UniFFI's RustBuffer).
            let data = if len > 0 && !rb.data.is_null() {
                std::slice::from_raw_parts(rb.data, len).to_vec()
            } else {
                Vec::new()
            };

            // Free the original RustBuffer — we have our own copy now.
            // `rustbuffer_free` is a pure C function, safe to call from any thread.
            napi_utils::free_rustbuffer(rb, rb_free_ptr);

            Some(RawCallbackArg::RustBuffer(data))
        }
        FfiTypeDesc::Callback(_) | FfiTypeDesc::MutReference(_) | FfiTypeDesc::Reference(_) => {
            let ptr = *(arg_ptr as *const *const c_void);
            Some(RawCallbackArg::Pointer(ptr as usize))
        }
        _ => None,
    }
}

/// Convert a [`RawCallbackArg`] into a JS value.
///
/// This is the second phase of the cross-thread conversion pipeline:
/// `C pointer -> RawCallbackArg -> JS value`. It runs on the main thread
/// inside the [`ThreadsafeFunction`] callback.
///
/// Scalar types are mapped to their natural JS representations (Number for
/// integers up to 32 bits, BigInt for 64-bit integers, Number/double for
/// floats). `RustBuffer` data is copied into a new JS `Uint8Array`.
///
/// # Panics
///
/// This function must only be called on the main Node.js thread. Calling it
/// off-thread will produce invalid napi values and likely crash.
pub fn raw_arg_to_js(env: &Env, raw_arg: &RawCallbackArg) -> napi::Result<napi::JsUnknown> {
    match raw_arg {
        RawCallbackArg::UInt8(v) => Ok(env.create_uint32(*v as u32)?.into_unknown()),
        RawCallbackArg::Int8(v) => Ok(env.create_int32(*v as i32)?.into_unknown()),
        RawCallbackArg::UInt16(v) => Ok(env.create_uint32(*v as u32)?.into_unknown()),
        RawCallbackArg::Int16(v) => Ok(env.create_int32(*v as i32)?.into_unknown()),
        RawCallbackArg::UInt32(v) => Ok(env.create_uint32(*v)?.into_unknown()),
        RawCallbackArg::Int32(v) => Ok(env.create_int32(*v)?.into_unknown()),
        RawCallbackArg::UInt64(v) => Ok(env.create_bigint_from_u64(*v)?.into_unknown()?),
        RawCallbackArg::Int64(v) => Ok(env.create_bigint_from_i64(*v)?.into_unknown()?),
        RawCallbackArg::Float32(v) => Ok(env.create_double(*v as f64)?.into_unknown()),
        RawCallbackArg::Float64(v) => Ok(env.create_double(*v)?.into_unknown()),
        RawCallbackArg::RustBuffer(data) => {
            let raw_env = env.raw();
            // SAFETY: `raw_env` is a valid napi_env because this function is
            // only called on the main thread (via the ThreadsafeFunction callback).
            // `create_uint8array` copies `data` into a new JS ArrayBuffer, so
            // the source slice only needs to be valid for the duration of this
            // call — which it is, since `data` is an owned `Vec<u8>`.
            let typedarray =
                unsafe { napi_utils::create_uint8array(raw_env, data.as_ptr(), data.len())? };
            // SAFETY: `raw_env` is valid (main thread) and `typedarray` is a
            // live `napi_value` just created above.
            Ok(unsafe { napi::JsUnknown::from_raw(raw_env, typedarray)? })
        }
        RawCallbackArg::Pointer(v) => Ok(env.create_bigint_from_u64(*v as u64)?.into_unknown()?),
        RawCallbackArg::StructBytes(data) => {
            let raw_env = env.raw();
            // SAFETY: `raw_env` is valid (main thread). `data` is an owned
            // `Vec<u8>` that outlives this call.
            let typedarray =
                unsafe { napi_utils::create_uint8array(raw_env, data.as_ptr(), data.len())? };
            // SAFETY: `raw_env` is valid and `typedarray` is a live napi_value.
            Ok(unsafe { napi::JsUnknown::from_raw(raw_env, typedarray)? })
        }
    }
}

/// Convert a JS return value into a [`RawCallbackArg`] for cross-thread
/// transport back to the calling thread.
///
/// This is the inverse of [`raw_arg_to_js`]: given a JS value produced by
/// calling a JS function on the main thread, pack it into a `RawCallbackArg`
/// so it can be sent back to the (possibly non-main) thread that initiated
/// the VTable call.
///
/// # Safety (of the `unsafe` block within)
///
/// The `from_raw` calls reconstruct napi wrapper types (`JsNumber`,
/// `JsBigInt`) from raw `napi_value` handles. This is safe because:
/// - We are on the main thread (this function is called from ThreadsafeFunction callbacks).
/// - `js_val` is a live `napi_value` returned from a JS function call
///   moments ago, so it has not been garbage-collected.
/// - `raw_env` is the current environment's raw handle.
pub fn js_return_to_raw(
    env: &Env,
    ret_type: &FfiTypeDesc,
    js_val: napi::JsUnknown,
) -> Option<RawCallbackArg> {
    let raw_env = env.raw();
    // SAFETY: All `from_raw` calls below reconstruct napi wrapper types from
    // `raw_env` (valid — we are on the main thread) and `js_val.raw()` (a
    // live napi_value from a just-completed JS function call).
    unsafe {
        match ret_type {
            FfiTypeDesc::Int8 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::Int8(num.get_double().ok()? as i8))
            }
            FfiTypeDesc::UInt8 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::UInt8(num.get_double().ok()? as u8))
            }
            FfiTypeDesc::Int16 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::Int16(num.get_double().ok()? as i16))
            }
            FfiTypeDesc::UInt16 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::UInt16(num.get_double().ok()? as u16))
            }
            FfiTypeDesc::Int32 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::Int32(num.get_double().ok()? as i32))
            }
            FfiTypeDesc::UInt32 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::UInt32(num.get_double().ok()? as u32))
            }
            FfiTypeDesc::Int64 => {
                let bigint = napi::JsBigInt::from_raw(raw_env, js_val.raw()).ok()?;
                let (v, _) = bigint.get_i64().ok()?;
                Some(RawCallbackArg::Int64(v))
            }
            FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
                let bigint = napi::JsBigInt::from_raw(raw_env, js_val.raw()).ok()?;
                let (v, _) = bigint.get_u64().ok()?;
                Some(RawCallbackArg::UInt64(v))
            }
            FfiTypeDesc::Float32 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::Float32(num.get_double().ok()? as f32))
            }
            FfiTypeDesc::Float64 => {
                let num = napi::JsNumber::from_raw(raw_env, js_val.raw()).ok()?;
                Some(RawCallbackArg::Float64(num.get_double().ok()?))
            }
            FfiTypeDesc::RustBuffer => {
                // Read the Uint8Array's underlying data pointer so we can copy
                // the bytes into an owned Vec for cross-thread transport.
                let (data, length) = napi_utils::read_typedarray_data(raw_env, js_val.raw())?;
                // SAFETY: `data` points to the JS TypedArray's backing store,
                // which remains valid for the duration of this synchronous call.
                let bytes = if length > 0 && !data.is_null() {
                    std::slice::from_raw_parts(data, length).to_vec()
                } else {
                    Vec::new()
                };
                Some(RawCallbackArg::RustBuffer(bytes))
            }
            _ => None,
        }
    }
}

/// Read a C argument from a raw pointer and convert it directly to a JS value.
///
/// This is the **same-thread fast path** used by [`trampoline_main_thread`].
/// Unlike the cross-thread path (which goes through [`read_raw_arg`] then
/// [`raw_arg_to_js`]), this function skips the intermediate `RawCallbackArg`
/// and creates napi values directly from the C pointers — one fewer copy for
/// `RustBuffer` data.
///
/// # Safety
///
/// - Must be called on the main Node.js thread (napi calls require it).
/// - `arg_ptr` must point to a value whose type matches `desc`. Guaranteed by
///   libffi when the CIF was built from the same `FfiTypeDesc` list.
/// - For `RustBuffer`: `arg_ptr` must point to a valid `RustBufferC`. After
///   this call returns, the original buffer is freed and must not be reused.
/// - `rb_free_ptr` must be a valid pointer to `rustbuffer_free`.
pub unsafe fn c_arg_to_js(
    env: &Env,
    desc: &FfiTypeDesc,
    arg_ptr: *const c_void,
    rb_free_ptr: *const c_void,
) -> napi::Result<napi::JsUnknown> {
    match desc {
        // SAFETY (all scalar arms): The CIF was built from the same FfiTypeDesc,
        // so libffi guarantees arg_ptr points to a correctly-typed, correctly-
        // aligned value. Each dereference reads a Copy type.
        FfiTypeDesc::UInt8 => {
            let v = *(arg_ptr as *const u8);
            Ok(env.create_uint32(v as u32)?.into_unknown())
        }
        FfiTypeDesc::Int8 => {
            let v = *(arg_ptr as *const i8);
            Ok(env.create_int32(v as i32)?.into_unknown())
        }
        FfiTypeDesc::UInt16 => {
            let v = *(arg_ptr as *const u16);
            Ok(env.create_uint32(v as u32)?.into_unknown())
        }
        FfiTypeDesc::Int16 => {
            let v = *(arg_ptr as *const i16);
            Ok(env.create_int32(v as i32)?.into_unknown())
        }
        FfiTypeDesc::UInt32 => {
            let v = *(arg_ptr as *const u32);
            Ok(env.create_uint32(v)?.into_unknown())
        }
        FfiTypeDesc::Int32 => {
            let v = *(arg_ptr as *const i32);
            Ok(env.create_int32(v)?.into_unknown())
        }
        FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
            let v = *(arg_ptr as *const u64);
            Ok(env.create_bigint_from_u64(v)?.into_unknown()?)
        }
        FfiTypeDesc::Int64 => {
            let v = *(arg_ptr as *const i64);
            Ok(env.create_bigint_from_i64(v)?.into_unknown()?)
        }
        FfiTypeDesc::Float32 => {
            let v = *(arg_ptr as *const f32);
            Ok(env.create_double(v as f64)?.into_unknown())
        }
        FfiTypeDesc::Float64 => {
            let v = *(arg_ptr as *const f64);
            Ok(env.create_double(v)?.into_unknown())
        }
        FfiTypeDesc::RustBuffer => {
            // SAFETY: arg_ptr points to a valid RustBufferC (guaranteed by CIF).
            let rb = *(arg_ptr as *const RustBufferC);
            let len = usize::try_from(rb.len).map_err(|_| {
                napi_utils::free_rustbuffer(rb, rb_free_ptr);
                napi::Error::from_reason("RustBuffer len exceeds addressable memory")
            })?;
            let raw_env = env.raw();

            // SAFETY: `create_uint8array` borrows `rb.data` to copy into a new
            // JS ArrayBuffer. The data is valid here because we have not yet
            // freed the buffer. `raw_env` is valid because we are on the main
            // thread.
            let typedarray = napi_utils::create_uint8array(raw_env, rb.data, len)?;

            // Free the original RustBuffer. The JS Uint8Array now owns its own
            // copy of the data, so the C-side buffer is no longer needed.
            napi_utils::free_rustbuffer(rb, rb_free_ptr);

            // SAFETY: `raw_env` is valid (main thread) and `typedarray` is a
            // live napi_value just created above.
            Ok(napi::JsUnknown::from_raw(raw_env, typedarray)?)
        }
        FfiTypeDesc::Callback(_) | FfiTypeDesc::MutReference(_) | FfiTypeDesc::Reference(_) => {
            let ptr = *(arg_ptr as *const *const c_void);
            Ok(env.create_bigint_from_u64(ptr as u64)?.into_unknown()?)
        }
        _ => Err(napi::Error::from_reason(format!(
            "Unsupported callback arg type: {desc:?}"
        ))),
    }
}

/// Build a libffi [`Cif`] (Call Interface) from a [`CallbackDef`].
///
/// The CIF describes the calling convention, argument types, and return type
/// of the callback to libffi. It is used both when creating the closure
/// (so libffi knows how to set up the trampoline's stack frame) and
/// implicitly at call time (so libffi can correctly unpack the arguments
/// into the `args` array that [`trampoline_callback`] receives).
///
/// The type mapping is handled by [`ffi_type_for`], which converts each
/// [`FfiTypeDesc`] variant to the corresponding libffi [`Type`].
pub fn build_callback_cif(
    callback_def: &CallbackDef,
    struct_defs: &HashMap<String, StructDef>,
) -> napi::Result<Cif> {
    let cif_arg_types: Vec<Type> = callback_def
        .args
        .iter()
        .map(|a| ffi_type_for(a, struct_defs))
        .collect::<napi::Result<Vec<_>>>()?;
    let cif_ret_type = ffi_type_for(&callback_def.ret, struct_defs)?;
    Ok(Cif::new(cif_arg_types, cif_ret_type))
}
