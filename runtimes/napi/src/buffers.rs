/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! How a `RustBuffer` crosses between Rust and JS, and who frees it.
//!
//! JS calls `rustbuffer_alloc` and `rustbuffer_free` ([`alloc_aliased`], [`alloc_copied`],
//! [`free`]); the call paths hand returned buffers to JS ([`BufferViews::hand_off`]) and
//! lower arguments from JS ([`BufferViews::lower`]). All four read or write a hidden marker
//! on the view ([`CapacitySymbol`]), whose value is what is left for the runtime to free:
//!
//! - **absent**: an ordinary V8-backed `Uint8Array` from JS, not the library's. Lowering
//!   copies it and `rustbuffer_free` refuses it.
//! - **greater than `0`**: a view over a live library allocation of that capacity, from
//!   `rustbuffer_alloc` or a returned buffer. Lowering adopts it and `rustbuffer_free`
//!   releases it. A returned view's `byteLength` is the buffer's `len`, so converters see
//!   only the message bytes; the marker carries the capacity the allocator needs.
//! - **`0`**: nothing is left to free, so `rustbuffer_free` does nothing. Either the
//!   allocation was adopted by a callee or released, or the view is V8 memory the runtime
//!   handed out: an empty return, or, under [`BufferViews::Copy`], every `rustbuffer_alloc`
//!   view and every return. Lowering refuses a non-empty one under [`BufferViews::Alias`],
//!   because it can only be a dangling adopted view, and copies it under
//!   [`BufferViews::Copy`], because nothing is adopted there.

use std::ffi::c_void;

use napi::{JsUnknown, NapiRaw, NapiValue};
use uniffi_runtime_core::ffi_c_types::RustBufferC;
use uniffi_runtime_core::Module;

use crate::core_err;
use crate::napi_utils::{self, CapacitySymbol};

/// Whether JS may view Rust memory, decided once per registration.
pub(crate) enum BufferViews {
    /// JS views Rust memory directly, as on Node.
    Alias,
    /// JS may not view Rust memory, so buffers are copied, as under Electron's memory cage.
    Copy,
}

impl BufferViews {
    /// Ask whether a JS `ArrayBuffer` may view memory it did not allocate.
    ///
    /// Node lets it. Electron does not: V8's memory cage keeps every `ArrayBuffer` inside
    /// the cage, so `napi_create_external_arraybuffer` answers
    /// `napi_no_external_buffers_allowed`. The answer is fixed for the life of the process.
    ///
    /// # Safety
    ///
    /// `raw_env` must be a valid `napi_env` for the current callback scope.
    pub(crate) unsafe fn probe(raw_env: napi::sys::napi_env) -> napi::Result<Self> {
        extern "C" fn free_probe(_env: napi::sys::napi_env, data: *mut c_void, _hint: *mut c_void) {
            // SAFETY: `data` is the `Box<u8>` leaked below; napi runs a finalizer at most
            // once, and only for a buffer it accepted.
            drop(unsafe { Box::from_raw(data.cast::<u8>()) });
        }

        // A one-byte allocation rather than a static, so the finalizer owns something real.
        let probe = Box::into_raw(Box::new(0u8));
        let mut arraybuffer = std::ptr::null_mut();
        // SAFETY: raw_env is valid (precondition); `probe` is a live one-byte allocation,
        // and `free_probe` releases it once V8 collects the unreachable buffer.
        let status = unsafe {
            napi::sys::napi_create_external_arraybuffer(
                raw_env,
                probe.cast(),
                1,
                Some(free_probe),
                std::ptr::null_mut(),
                &mut arraybuffer,
            )
        };
        if status == napi::sys::Status::napi_ok {
            return Ok(Self::Alias);
        }
        // SAFETY: the buffer was refused, so napi never took `probe` and will not run the
        // finalizer; this is its only owner.
        drop(unsafe { Box::from_raw(probe) });
        if status == napi::sys::Status::napi_no_external_buffers_allowed {
            Ok(Self::Copy)
        } else {
            Err(napi::Error::from_reason(
                "Failed to probe for external ArrayBuffer support".to_string(),
            ))
        }
    }

    /// Hand a returned `RustBufferC` to JS as a `Uint8Array`.
    ///
    /// Under [`Self::Alias`] the view aliases the Rust-owned bytes: no boundary copy. The
    /// codegen-emitted lift wrapper calls `converter.lift(view)` inside a `try/finally` and
    /// invokes the runtime's `rustbuffer_free(view)` afterwards, so the single mandatory copy
    /// happens inside `lift()` itself (via `dest.set(view)` for byte arrays,
    /// `TextDecoder.decode` for strings, field-by-field reads for composites).
    ///
    /// Under [`Self::Copy`], and for an empty buffer, the bytes are copied into V8 memory
    /// and the Rust allocation is released at once.
    ///
    /// On any error in the handoff, we free the buffer to avoid leaking the
    /// Rust-side allocation.
    pub(crate) fn hand_off(
        &self,
        env: &napi::Env,
        rb: RustBufferC,
        rb_free_ptr: *const c_void,
        capacity_symbol: &CapacitySymbol,
    ) -> napi::Result<JsUnknown> {
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

        // Copy instead of aliasing, and release the allocation here, when there is nothing
        // to alias or JS may not view Rust memory. A zero-length typed array cannot carry the
        // data pointer forward, leaving `rustbuffer_free` nothing to work from.
        let has_bytes = rb.len > 0 && rb.capacity > 0 && !rb.data.is_null();
        if !has_bytes || matches!(self, Self::Copy) {
            let copy_len = if has_bytes { len } else { 0 };
            // SAFETY: `raw_env` is valid for this callback scope. Either `rb.data` holds at
            // least `rb.len` readable bytes, or `copy_len` is 0 and nothing is read.
            let typedarray =
                match unsafe { napi_utils::create_uint8array(raw_env, rb.data, copy_len) } {
                    Ok(typedarray) => typedarray,
                    Err(error) => {
                        // SAFETY: as above — `rb_free_ptr` was resolved at registration time
                        // and `rb` has had no other owner take it yet.
                        unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
                        return Err(error);
                    }
                };
            // SAFETY: as above — `typedarray` owns its own V8 memory, so nothing aliases
            // `rb.data` regardless of which condition tripped the branch; `rb` has had no
            // other owner take it.
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
        let typedarray =
            match unsafe { napi_utils::create_external_uint8array(raw_env, rb.data, len) } {
                Ok(typedarray) => typedarray,
                Err(error) => {
                    // SAFETY: as above — `rb_free_ptr` was resolved at registration time
                    // and `rb` has had no other owner take it yet.
                    unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
                    return Err(error);
                }
            };

        // Mark every handed-off view, including when `capacity == byteLength`: an absent
        // marker means "not the library's, do not free", so an unmarked view leaks.
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

    /// Convert a JS `Uint8Array` argument to a [`RustBufferC`] the callee can consume.
    ///
    /// Two kinds of view arrive here, and they must be handled differently:
    ///
    /// - **Library-owned views**, from `rustbuffer_alloc` under [`Self::Alias`]. Codegen
    ///   allocates one of these, fills it in place, and passes it straight through as the
    ///   argument. Nothing else will ever free it: codegen frees returned buffers but never
    ///   lowered arguments, and the view has a no-op finalizer. These are **adopted** — we
    ///   hand the existing allocation to the callee, which frees it. Copying such a view
    ///   instead would orphan the original and leak one whole payload per call.
    /// - **V8-owned arrays**: any ordinary `Uint8Array` from JS, and under [`Self::Copy`]
    ///   every view the runtime hands out. These are not ours to give away, so they are
    ///   **copied** into a fresh library allocation via `rustbuffer_from_bytes`.
    ///
    /// The marker is the discriminator: the runtime knows a buffer's capacity precisely when
    /// the library allocated it. On adoption the marker is reset to `0`, which makes any later
    /// `rustbuffer_free(view)` a no-op rather than a double free.
    ///
    /// The symbol is a required parameter, deliberately. It used to be an `Option`, and a
    /// caller with no symbol in scope could pass `None` to force the copy path — memory-safe
    /// in itself, but it silently orphaned every library-owned view that reached it, because
    /// nothing else frees a lowered argument. Requiring the symbol makes that a compile error
    /// instead of a leak, so a marshalling path added later cannot opt out by accident.
    ///
    /// # Safety
    ///
    /// - `raw_env` must be a valid `napi_env` for the current callback scope.
    /// - `js_val` must be a `napi_value` referring to a JS `TypedArray`.
    /// - `rb_from_bytes_ptr` must point to a valid `rustbuffer_from_bytes` function.
    pub(crate) unsafe fn lower(
        &self,
        raw_env: napi::sys::napi_env,
        js_val: JsUnknown,
        rb_from_bytes_ptr: *const c_void,
        capacity_symbol: &CapacitySymbol,
    ) -> napi::Result<RustBufferC> {
        // SAFETY: `raw()` drops the handle's lifetime tracking, so it needs the value
        // to be live for the rest of this call — `js_val` is owned here and only read
        // below, within the callback scope this function's contract names.
        let raw_val = unsafe { js_val.raw() };
        // SAFETY: raw_env is valid and raw_val refers to a TypedArray, per this function's contract.
        let (data_ptr, length) = unsafe { napi_utils::read_typedarray_data(raw_env, raw_val) }
            .ok_or_else(|| {
                napi::Error::from_reason(
                    "Expected a Uint8Array argument for RustBuffer".to_string(),
                )
            })?;
        // SAFETY: data_ptr/length came from read_typedarray_data above; rb_from_bytes_ptr is
        // a valid rustbuffer_from_bytes per this function's contract.
        let copy_bytes = || unsafe {
            napi_utils::rustbuffer_from_raw_bytes(data_ptr, length, rb_from_bytes_ptr)
        };

        // An empty view owns no allocation, so there is nothing to adopt or refuse.
        if length == 0 {
            return copy_bytes();
        }

        // SAFETY: raw_env is valid and raw_val is a JS object (a TypedArray), per this function's contract.
        match unsafe { capacity_symbol.get(raw_env, raw_val) }? {
            // No marker: an ordinary V8-backed array from JS, which is not ours to give away.
            None => copy_bytes(),
            // The marker exists but has been zeroed, so this view was already adopted and
            // its memory has since been freed by the callee. Reading it would be a
            // use-after-free, so refuse rather than hand the callee a dangling pointer.
            Some(0) if matches!(self, Self::Alias) => Err(napi::Error::from_reason(
                "RustBuffer argument was already consumed by a previous FFI call".to_string(),
            )),
            // Nothing is adopted here, so a zeroed marker means live V8 memory the runtime
            // handed out, which is not ours to give away either.
            Some(0) => copy_bytes(),
            Some(capacity) => {
                // SAFETY: raw_env is valid and raw_val is a JS object (a TypedArray), per
                // this function's contract.
                unsafe { capacity_symbol.set(raw_env, raw_val, 0) }?;
                Ok(RustBufferC {
                    capacity,
                    len: length as u64,
                    data: data_ptr as *mut u8,
                })
            }
        }
    }
}

/// `rustbuffer_alloc(size)` where JS may view Rust memory: a view over a new library
/// allocation, marked with its capacity.
///
/// Codegen fills the view in place and passes it as an argument, where lowering adopts it.
/// Adoption frees the backing allocation but cannot detach the view, so a view must be
/// treated as consumed once it has been passed: reading it afterwards is a use-after-free,
/// and passing it a second time fails with an "already consumed" error. Generated lowering
/// code drops the view immediately, so this only bites hand-written callers.
///
/// # Safety
///
/// `raw_env` must be a valid `napi_env` for the current callback scope.
pub(crate) unsafe fn alloc_aliased(
    raw_env: napi::sys::napi_env,
    module: &Module,
    size: i32,
    capacity_symbol: &CapacitySymbol,
) -> napi::Result<JsUnknown> {
    check_size(size)?;
    let rb = module.rustbuffer_alloc(size).map_err(core_err)?;
    let len = usize::try_from(rb.capacity).map_err(|_| {
        // Free what we just allocated to avoid leaking on the error path.
        // SAFETY: `rb` was just allocated above and not yet handed to JS, so this is
        // the sole owner; `free_ptr` was resolved at registration time.
        unsafe { napi_utils::free_rustbuffer(rb, module.rb_ops().free_ptr) };
        napi::Error::from_reason("RustBuffer capacity exceeds addressable memory".to_string())
    })?;
    // SAFETY: rb.data points to a valid allocation of `len` bytes that the Rust
    // library owns; the view is released either by `rustbuffer_free(view)` or by being
    // adopted when passed as an FFI argument, so no finalizer is required.
    let typedarray = unsafe { napi_utils::create_external_uint8array(raw_env, rb.data, len)? };
    // Mark unconditionally, including `rustbuffer_alloc(0)` where the capacity is 0: an
    // absent marker means "not the library's, refuse to free", so a zero-capacity view is
    // marked 0 rather than left bare.
    //
    // SAFETY: `raw_env` is the active env for this callback, and `typedarray` is the
    // view created just above.
    unsafe { capacity_symbol.set(raw_env, typedarray, rb.capacity)? };
    // SAFETY: `raw_env` is the active env for this callback, and `typedarray` is the
    // live napi_value created above.
    unsafe { JsUnknown::from_raw(raw_env, typedarray) }
}

/// `rustbuffer_alloc(size)` where JS may not view Rust memory: V8 memory marked 0, so there
/// is nothing for `rustbuffer_free` to release and lowering copies it. Rust never allocates.
///
/// # Safety
///
/// `raw_env` must be a valid `napi_env` for the current callback scope.
pub(crate) unsafe fn alloc_copied(
    raw_env: napi::sys::napi_env,
    size: i32,
    capacity_symbol: &CapacitySymbol,
) -> napi::Result<JsUnknown> {
    let len = check_size(size)?;
    // SAFETY: `raw_env` is the active env for this callback; a null source allocates `len`
    // bytes without reading anything.
    let typedarray = unsafe { napi_utils::create_uint8array(raw_env, std::ptr::null(), len)? };
    // SAFETY: `raw_env` is the active env for this callback, and `typedarray` is the view
    // created just above.
    unsafe { capacity_symbol.set(raw_env, typedarray, 0)? };
    // SAFETY: `raw_env` is the active env for this callback, and `typedarray` is the
    // live napi_value created above.
    unsafe { JsUnknown::from_raw(raw_env, typedarray) }
}

fn check_size(size: i32) -> napi::Result<usize> {
    usize::try_from(size).map_err(|_| {
        napi::Error::from_reason("rustbuffer_alloc size must be non-negative".to_string())
    })
}

/// `rustbuffer_free(view)`: release whatever the marker says is left to free.
///
/// Rejecting an unmarked view rather than ignoring it keeps a caller's mistake visible.
/// Guessing the capacity from `byteLength` instead would pass V8-owned memory to Rust's
/// allocator.
///
/// # Safety
///
/// - `raw_env` must be a valid `napi_env` for the current callback scope.
/// - `rb_free_ptr` must point to the library's `rustbuffer_free`, resolved at registration.
pub(crate) unsafe fn free(
    raw_env: napi::sys::napi_env,
    js_val: JsUnknown,
    rb_free_ptr: *const c_void,
    capacity_symbol: &CapacitySymbol,
) -> napi::Result<()> {
    // SAFETY: js_val is a JS value from the current callback scope; `raw()`
    // returns the underlying napi_value handle without changing its ownership.
    let raw_val = unsafe { js_val.raw() };
    // SAFETY: js_val came from JS; if it is not a typed array,
    // `read_typedarray_data` returns None and we surface a clean error.
    let (data_ptr, _) =
        unsafe { napi_utils::read_typedarray_data(raw_env, raw_val) }.ok_or_else(|| {
            napi::Error::from_reason("rustbuffer_free expected a Uint8Array argument".to_string())
        })?;
    // SAFETY: raw_env / raw_val are valid for the current callback scope, and
    // `raw_val` is the typed array the caller passed.
    let capacity = unsafe { capacity_symbol.get(raw_env, raw_val)? }.ok_or_else(|| {
        napi::Error::from_reason("rustbuffer_free received an unowned Uint8Array")
    })?;
    if capacity == 0 {
        return Ok(());
    }

    // Mark the view released before handing its allocation back, so repeated
    // cleanup is a no-op rather than a double free.
    //
    // SAFETY: as above — `raw_env` and `raw_val` are valid for this callback scope.
    unsafe { capacity_symbol.set(raw_env, raw_val, 0)? };
    let rb = RustBufferC {
        capacity,
        len: 0,
        data: data_ptr as *mut u8,
    };
    // SAFETY: rb_free_ptr was resolved at registration time, and the marker ties this
    // exact pointer and capacity to a buffer the library allocated.
    unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
    Ok(())
}
