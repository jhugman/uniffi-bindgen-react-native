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
//! appear throughout the crate — in callback argument marshalling, return value
//! handling, error buffer extraction, and `RustBuffer` conversion — making
//! shared helpers essential.

use std::ffi::c_void;

use napi::{JsUnknown, NapiRaw};

use crate::ffi_c_types::{
    ForeignBytesC, RustBufferC, RustBufferFreeFn, RustBufferFromBytesFn, RustCallStatusC,
};

/// Create a JS `Uint8Array` from raw bytes.
///
/// The two-step construction — first an `ArrayBuffer`, then a `TypedArray` view
/// over it — is the only way to create a `Uint8Array` via the napi C API.
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
/// alive** — that is, within the current napi callback scope. Callers must
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
    // ArrayBuffer's backing store, which the JS engine manages — we do not
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
/// This is the shared core of all "bytes → RustBuffer" conversions in the crate.
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
    // was provided under `symbols.rustbufferFromBytes` in the JS definitions. We
    // transmute it to `RustBufferFromBytesFn` — the correct signature for UniFFI's
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
/// The triple guard — data not null, capacity > 0, free pointer not null —
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
