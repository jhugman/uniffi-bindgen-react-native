/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Low-level napi helpers.
//!
//! This module provides thin wrappers around the raw napi C API for operations
//! that the napi-rs high-level API does not expose directly. These functions
//! operate on raw `napi_env` and `napi_value` handles, bypassing napi-rs's type
//! system to perform buffer operations that would otherwise require verbose,
//! duplicated boilerplate.
//!
//! # Why raw napi?
//!
//! The napi-rs crate provides safe wrappers for most operations, but its
//! TypedArray support is limited. Creating a `Uint8Array` from raw bytes
//! requires three raw napi calls in sequence (create `ArrayBuffer`, copy data,
//! create `TypedArray` view), and reading TypedArray data requires a
//! five-out-parameter call to `napi_get_typedarray_info`. These operations
//! appear throughout the crate—in callback argument marshalling, return value
//! handling, error buffer extraction, and `RustBuffer` conversion—making
//! shared helpers essential.

use std::ffi::c_void;

use napi::{JsUnknown, NapiRaw};

use uniffi_runtime_core::ffi_c_types::{
    ForeignBytesC, RustBufferAllocFn, RustBufferC, RustBufferFreeFn, RustBufferFromBytesFn,
    RustCallStatusC,
};

/// A per-registration JS Symbol used as a hidden property key for stashing the
/// underlying RustBuffer capacity on a lift-handoff `Uint8Array`. The view's
/// `byteLength` is set to `len` so converters that decode the whole view
/// (strings, raw byte arrays) see only the message bytes — but the global
/// allocator needs `capacity` to free correctly when `capacity > len`. The
/// symbol is created once when the module registers and lives as long as the
/// JS-side module facade.
pub struct CapacitySymbol {
    /// `napi_ref` keeping the Symbol alive across JS callbacks. Symbols are GC-
    /// managed; we hold a strong reference (initial refcount 1) so the symbol
    /// remains valid for the lifetime of this struct.
    sym_ref: napi::sys::napi_ref,
    /// Captured at creation time so `Drop` can release the reference against
    /// the env that owns it.
    raw_env: napi::sys::napi_env,
}

// SAFETY: `napi_ref` and `napi_env` are raw pointers that the napi runtime
// manages. They're only ever dereferenced from the JS thread (where this
// `CapacitySymbol` is constructed and used). `Send`/`Sync` is required because
// the symbol is parked inside an `Arc` shared with closures captured by
// `env.create_function_from_closure`, and napi-rs requires those closures to
// be `Send`. The pointers themselves never cross threads.
unsafe impl Send for CapacitySymbol {}
unsafe impl Sync for CapacitySymbol {}

impl CapacitySymbol {
    /// Create a new capacity-hint Symbol with a descriptive label and stash a
    /// reference to it.
    ///
    /// # Safety
    ///
    /// `raw_env` must be a valid `napi_env` for the current callback scope.
    pub unsafe fn new(raw_env: napi::sys::napi_env) -> napi::Result<Self> {
        // Create a JS string for the symbol's description (debug aid only).
        let desc = b"uniffi.rustbuffer.capacity\0";
        let mut desc_val: napi::sys::napi_value = std::ptr::null_mut();
        let status = napi::sys::napi_create_string_utf8(
            raw_env,
            desc.as_ptr() as *const _,
            desc.len() - 1,
            &mut desc_val,
        );
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to create symbol description string".to_string(),
            ));
        }

        let mut sym_val: napi::sys::napi_value = std::ptr::null_mut();
        let status = napi::sys::napi_create_symbol(raw_env, desc_val, &mut sym_val);
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to create capacity-hint Symbol".to_string(),
            ));
        }

        // Keep the symbol alive past the current callback scope.
        let mut sym_ref: napi::sys::napi_ref = std::ptr::null_mut();
        let status = napi::sys::napi_create_reference(raw_env, sym_val, 1, &mut sym_ref);
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to create reference to capacity-hint Symbol".to_string(),
            ));
        }
        Ok(Self { sym_ref, raw_env })
    }

    /// Resolve the held reference to the live `napi_value` for the symbol.
    ///
    /// # Safety
    ///
    /// `raw_env` must match the env this symbol was created in (any active
    /// callback scope on the same JS thread satisfies this).
    pub unsafe fn value(
        &self,
        raw_env: napi::sys::napi_env,
    ) -> napi::Result<napi::sys::napi_value> {
        let mut sym_val: napi::sys::napi_value = std::ptr::null_mut();
        let status = napi::sys::napi_get_reference_value(raw_env, self.sym_ref, &mut sym_val);
        if status != napi::sys::Status::napi_ok || sym_val.is_null() {
            return Err(napi::Error::from_reason(
                "Failed to resolve capacity-hint Symbol".to_string(),
            ));
        }
        Ok(sym_val)
    }

    /// Stash `capacity` on `obj` under the symbol-keyed hidden property.
    ///
    /// # Safety
    ///
    /// `raw_env` must be valid; `obj` must be a JS object (a `Uint8Array` is
    /// an object as far as napi property access is concerned).
    pub unsafe fn set(
        &self,
        raw_env: napi::sys::napi_env,
        obj: napi::sys::napi_value,
        capacity: u64,
    ) -> napi::Result<()> {
        let sym_val = self.value(raw_env)?;
        // Use a BigInt to hold the full u64 capacity without lossy f64 rounding
        // (matters for the >2^53 corner case; cheap enough in practice).
        let mut cap_val: napi::sys::napi_value = std::ptr::null_mut();
        let status = napi::sys::napi_create_bigint_uint64(raw_env, capacity, &mut cap_val);
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to create BigInt for capacity hint".to_string(),
            ));
        }
        let status = napi::sys::napi_set_property(raw_env, obj, sym_val, cap_val);
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to set capacity-hint property".to_string(),
            ));
        }
        Ok(())
    }

    /// Read the capacity hint from `obj`. Returns `None` if no hint is set
    /// (e.g., views from `rustbuffer_alloc(n)` where `byteLength == capacity`).
    ///
    /// # Safety
    ///
    /// `raw_env` must be valid; `obj` must be a JS object.
    pub unsafe fn get(
        &self,
        raw_env: napi::sys::napi_env,
        obj: napi::sys::napi_value,
    ) -> Option<u64> {
        let sym_val = self.value(raw_env).ok()?;
        let mut has = false;
        let status = napi::sys::napi_has_property(raw_env, obj, sym_val, &mut has);
        if status != napi::sys::Status::napi_ok || !has {
            return None;
        }
        let mut cap_val: napi::sys::napi_value = std::ptr::null_mut();
        let status = napi::sys::napi_get_property(raw_env, obj, sym_val, &mut cap_val);
        if status != napi::sys::Status::napi_ok || cap_val.is_null() {
            return None;
        }
        let mut value: u64 = 0;
        let mut lossless = false;
        let status =
            napi::sys::napi_get_value_bigint_uint64(raw_env, cap_val, &mut value, &mut lossless);
        if status != napi::sys::Status::napi_ok {
            return None;
        }
        Some(value)
    }
}

impl Drop for CapacitySymbol {
    fn drop(&mut self) {
        // SAFETY: `sym_ref` was created from `raw_env` by `napi_create_reference`
        // and has not been deleted yet. Releasing it tells napi the symbol is
        // free to GC.
        if !self.sym_ref.is_null() && !self.raw_env.is_null() {
            unsafe {
                napi::sys::napi_delete_reference(self.raw_env, self.sym_ref);
            }
        }
    }
}

/// Create a JS `Uint8Array` from raw bytes.
///
/// The two-step construction—first an `ArrayBuffer`, then a `TypedArray` view
/// over it—is the only way to create a `Uint8Array` via the napi C API.
/// The `ArrayBuffer` owns the backing memory; the `TypedArray` is a lightweight
/// view with zero additional allocation. We copy `len` bytes from `data` into
/// the newly allocated `ArrayBuffer` and return the `Uint8Array` view as a raw
/// `napi_value`.
///
/// # Safety
///
/// - `raw_env` must be a valid `napi_env` for the current callback scope.
/// - `data` must point to at least `len` readable bytes (ignored when `len == 0`).
pub unsafe fn create_uint8array(
    raw_env: napi::sys::napi_env,
    data: *const u8,
    len: usize,
) -> napi::Result<napi::sys::napi_value> {
    let mut arraybuffer_data: *mut c_void = std::ptr::null_mut();
    let mut arraybuffer = std::ptr::null_mut();
    // SAFETY: raw_env is valid (precondition); output pointers are to local
    // stack variables whose addresses remain stable for the duration of the call.
    let status =
        napi::sys::napi_create_arraybuffer(raw_env, len, &mut arraybuffer_data, &mut arraybuffer);
    if status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_reason("Failed to create ArrayBuffer"));
    }

    if len > 0 && !data.is_null() {
        // SAFETY: data points to at least `len` bytes (precondition);
        // arraybuffer_data points to the napi-allocated buffer of exactly `len`
        // bytes (guaranteed by napi_create_arraybuffer on success). The two
        // regions cannot overlap because one lives in Rust memory and the other
        // in the JS engine's heap.
        std::ptr::copy_nonoverlapping(data, arraybuffer_data as *mut u8, len);
    }

    let mut typedarray = std::ptr::null_mut();
    // SAFETY: arraybuffer is the napi_value just created above; byte_offset=0
    // and length=len match the ArrayBuffer's size, so the view covers exactly
    // the entire buffer with no out-of-bounds region.
    let status = napi::sys::napi_create_typedarray(
        raw_env,
        napi::sys::TypedarrayType::uint8_array,
        len,
        arraybuffer,
        0,
        &mut typedarray,
    );
    if status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_reason("Failed to create Uint8Array"));
    }

    Ok(typedarray)
}

/// Read the data pointer and byte length from a JS `TypedArray`.
///
/// On success, returns `Some((data_ptr, length))` where `data_ptr` points into
/// the `ArrayBuffer`'s backing store. **The returned pointer borrows the
/// ArrayBuffer's internal storage and is valid only while the JS value is
/// alive**—that is, within the current napi callback scope. Callers must
/// finish reading before returning control to the JS engine, because a GC cycle
/// could relocate or free the backing store afterward.
///
/// Returns `None` if `raw_val` is not a typed array (the napi call fails).
///
/// # Safety
///
/// - `raw_env` must be a valid `napi_env` for the current callback scope.
/// - `raw_val` must be a valid `napi_value` referring to a `TypedArray`.
pub unsafe fn read_typedarray_data(
    raw_env: napi::sys::napi_env,
    raw_val: napi::sys::napi_value,
) -> Option<(*const u8, usize)> {
    let mut length: usize = 0;
    let mut data: *mut c_void = std::ptr::null_mut();
    let mut ab = std::ptr::null_mut();
    let mut byte_offset: usize = 0;
    let mut ta_type: i32 = 0;
    // SAFETY: raw_env and raw_val are valid (precondition); output pointers
    // are to local stack variables. The returned data pointer is into the
    // ArrayBuffer's backing store, which the JS engine manages—we do not
    // free it ourselves.
    let status = napi::sys::napi_get_typedarray_info(
        raw_env,
        raw_val,
        &mut ta_type,
        &mut length,
        &mut data,
        &mut ab,
        &mut byte_offset,
    );
    if status != napi::sys::Status::napi_ok {
        return None;
    }
    Some((data as *const u8, length))
}

/// Allocate a [`RustBufferC`] by copying raw bytes through `rustbuffer_from_bytes`.
///
/// This is the shared core of all "bytes -> RustBuffer" conversions in the crate.
/// It wraps the provided bytes in a [`ForeignBytesC`] (a borrowed view) and calls
/// the library's `rustbuffer_from_bytes` to produce a Rust-owned copy.
///
/// # Safety
///
/// - `data` must point to at least `len` readable bytes (ignored when `len == 0`).
/// - `rb_from_bytes_ptr` must point to a valid `rustbuffer_from_bytes` function.
pub unsafe fn rustbuffer_from_raw_bytes(
    data: *const u8,
    len: usize,
    rb_from_bytes_ptr: *const c_void,
) -> napi::Result<RustBufferC> {
    if len > i32::MAX as usize {
        return Err(napi::Error::from_reason(
            "RustBuffer too large for ForeignBytes (max 2GB)".to_string(),
        ));
    }
    let foreign = ForeignBytesC {
        len: len as i32,
        data: if len > 0 { data } else { std::ptr::null() },
    };
    // SAFETY: `rb_from_bytes_ptr` was obtained via `dlsym` for the symbol whose name
    // was provided under `symbols.rustbuffer_from_bytes` in the JS definitions. We
    // transmute it to `RustBufferFromBytesFn`—the correct signature for UniFFI's
    // `rustbuffer_from_bytes`.
    let from_bytes: RustBufferFromBytesFn = std::mem::transmute(rb_from_bytes_ptr);
    let mut call_status = RustCallStatusC::default();
    let rb = from_bytes(foreign, &mut call_status as *mut RustCallStatusC);
    if call_status.code != 0 {
        return Err(napi::Error::from_reason(
            "rustbuffer_from_bytes failed".to_string(),
        ));
    }
    Ok(rb)
}

/// Allocate a [`RustBufferC`] of the requested capacity via `rustbuffer_alloc`.
///
/// Returned buffer has `capacity == size`, `len == 0`, and a heap-allocated `data`
/// pointer owned by the Rust library. Callers must hand the buffer back through the
/// matching `rustbuffer_free` (or pass it across the FFI to a function that consumes it).
///
/// # Safety
///
/// - `rb_alloc_ptr` must point to a valid `rustbuffer_alloc` function.
pub unsafe fn rustbuffer_alloc(
    size: i32,
    rb_alloc_ptr: *const c_void,
) -> napi::Result<RustBufferC> {
    if rb_alloc_ptr.is_null() {
        return Err(napi::Error::from_reason(
            "rustbuffer_alloc symbol is unresolved".to_string(),
        ));
    }
    // SAFETY: `rb_alloc_ptr` was obtained via `dlsym` for the symbol whose name was
    // provided under `symbols.rustbuffer_alloc` in the JS definitions. We transmute
    // it to `RustBufferAllocFn`—the correct signature for UniFFI's `rustbuffer_alloc`.
    let alloc: RustBufferAllocFn = std::mem::transmute(rb_alloc_ptr);
    let mut call_status = RustCallStatusC::default();
    let rb = alloc(size, &mut call_status as *mut RustCallStatusC);
    if call_status.code != 0 {
        return Err(napi::Error::from_reason(
            "rustbuffer_alloc failed".to_string(),
        ));
    }
    Ok(rb)
}

/// Wrap externally-managed memory as a JS `Uint8Array` (via napi external ArrayBuffer).
///
/// Unlike [`create_uint8array`], this **does not copy**: the returned typed array is a
/// view directly over `data`. The caller is responsible for keeping the memory alive
/// for as long as JS holds the view—we pass a no-op finalizer because the codegen-emitted
/// wrapper drops the JS reference before the corresponding `rustbuffer_free` runs.
///
/// # Safety
///
/// - `raw_env` must be a valid `napi_env` for the current callback scope.
/// - `data` must point to at least `len` readable+writable bytes that remain alive
///   for the lifetime of the returned typed array (ignored when `len == 0`).
pub unsafe fn create_external_uint8array(
    raw_env: napi::sys::napi_env,
    data: *mut u8,
    len: usize,
) -> napi::Result<napi::sys::napi_value> {
    // No-op finalizer: ownership stays with the Rust library; codegen will call
    // `rustbuffer_free` explicitly. Using `napi_create_external_arraybuffer` with
    // a null finalizer is technically permitted, but napi engines (Node 18+) emit
    // a warning. A trivial extern "C" callback that does nothing is safer.
    extern "C" fn noop_finalize(_env: napi::sys::napi_env, _data: *mut c_void, _hint: *mut c_void) {
    }

    let mut arraybuffer = std::ptr::null_mut();
    // SAFETY: raw_env is valid (precondition); `data`+`len` describe a valid byte
    // range we intend to expose; the finalizer is a static extern "C" function.
    let status = napi::sys::napi_create_external_arraybuffer(
        raw_env,
        data as *mut c_void,
        len,
        Some(noop_finalize),
        std::ptr::null_mut(),
        &mut arraybuffer,
    );
    if status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_reason(
            "Failed to create external ArrayBuffer".to_string(),
        ));
    }

    let mut typedarray = std::ptr::null_mut();
    // SAFETY: arraybuffer is the napi_value just created above; byte_offset=0
    // and length=len match the ArrayBuffer's size, so the view covers exactly
    // the entire buffer with no out-of-bounds region.
    let status = napi::sys::napi_create_typedarray(
        raw_env,
        napi::sys::TypedarrayType::uint8_array,
        len,
        arraybuffer,
        0,
        &mut typedarray,
    );
    if status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_reason(
            "Failed to create Uint8Array view".to_string(),
        ));
    }
    Ok(typedarray)
}

/// Convert a JS `Uint8Array` to a [`RustBufferC`] by reading its data and calling
/// [`rustbuffer_from_raw_bytes`].
///
/// # Safety
///
/// - `raw_env` must be a valid `napi_env` for the current callback scope.
/// - `js_val` must be a `napi_value` referring to a JS `TypedArray`.
/// - `rb_from_bytes_ptr` must point to a valid `rustbuffer_from_bytes` function.
pub unsafe fn js_uint8array_to_rust_buffer(
    raw_env: napi::sys::napi_env,
    js_val: JsUnknown,
    rb_from_bytes_ptr: *const c_void,
) -> napi::Result<RustBufferC> {
    let (data_ptr, length) = read_typedarray_data(raw_env, js_val.raw()).ok_or_else(|| {
        napi::Error::from_reason("Expected a Uint8Array argument for RustBuffer".to_string())
    })?;
    rustbuffer_from_raw_bytes(data_ptr, length, rb_from_bytes_ptr)
}

/// Free a [`RustBufferC`] via the provided free function pointer.
///
/// The triple guard—data not null, capacity > 0, free pointer not null—
/// prevents double-frees and no-ops for empty buffers. A zero-capacity buffer
/// was never allocated, so there is nothing to free; a null data pointer
/// signals the same. A null `free_ptr` occurs when the symbol could not be
/// resolved (e.g., during early initialization), and calling through it would
/// be undefined behavior.
///
/// # Safety
///
/// - `free_ptr`, if non-null, must point to a valid `rustbuffer_free` function
///   with the signature `extern "C" fn(RustBufferC, *mut RustCallStatusC)`.
/// - `rb` must have been allocated by the same library whose `rustbuffer_free`
///   is pointed to by `free_ptr`.
pub unsafe fn free_rustbuffer(rb: RustBufferC, free_ptr: *const c_void) {
    if !rb.data.is_null() && rb.capacity > 0 && !free_ptr.is_null() {
        // SAFETY: free_ptr was resolved via dlsym from a loaded library and has
        // the signature `extern "C" fn(RustBufferC, *mut RustCallStatusC)`.
        // The transmute reinterprets the raw `*const c_void` as a function
        // pointer of the correct type. The non-null check above guarantees we
        // are not transmuting a null pointer.
        let free_fn: RustBufferFreeFn = std::mem::transmute(free_ptr);
        let mut status = RustCallStatusC::default();
        // SAFETY: rb is a valid RustBufferC that was allocated by the same
        // library's rustbuffer_alloc or rustbuffer_from_bytes (precondition).
        // We pass a stack-allocated RustCallStatusC for the function to write
        // its status into; its address is valid for the duration of the call.
        free_fn(rb, &mut status as *mut _);
    }
}
