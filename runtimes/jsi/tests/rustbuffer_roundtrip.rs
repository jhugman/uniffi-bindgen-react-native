/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! C-ABI RustBuffer alloc/from_bytes/free round-trips against a real cdylib.

mod common;
use common::{fixture_cdylib, register_hello_world, TAG_UINT32};
use uniffi_runtime_jsi::{
    ubrn_jsi_call, ubrn_jsi_free, ubrn_jsi_register, ubrn_jsi_rustbuffer_alloc,
    ubrn_jsi_rustbuffer_free, ubrn_jsi_rustbuffer_from_bytes, UbrnFunctionSpec, UbrnModuleSpec,
};

/// Player tag name for a RustBuffer, from the wire vocabulary in
/// `runtimes/jsi/include/ubrn_jsi.h`.
const TAG_RUSTBUFFER: &str = "RustBuffer";

#[test]
fn alloc_then_free() {
    let m = register_hello_world();
    let rb = unsafe { ubrn_jsi_rustbuffer_alloc(m, 16) };
    assert!(!rb.data.is_null());
    assert!(rb.capacity >= 16);
    unsafe { ubrn_jsi_rustbuffer_free(m, rb) };
    unsafe { ubrn_jsi_free(m) };
}

#[test]
fn from_bytes_then_free() {
    let m = register_hello_world();
    let data: [u8; 5] = *b"hello";
    let rb = unsafe { ubrn_jsi_rustbuffer_from_bytes(m, data.as_ptr(), data.len()) };
    assert!(!rb.data.is_null());
    assert_eq!(rb.len, 5);
    unsafe { ubrn_jsi_rustbuffer_free(m, rb) };
    unsafe { ubrn_jsi_free(m) };
}

/// Register the hello-world fixture with the `describe` function exposed.
///
/// `describe(u32) -> String` is the simplest RustBuffer-returning function we
/// can test: one scalar arg, one RustBuffer return, and a RustCallStatus.
fn register_with_describe() -> *mut uniffi_runtime_jsi::UbrnJsiModule {
    let lib = std::ffi::CString::new(fixture_cdylib()).unwrap();
    let alloc = std::ffi::CString::new("ffi_hello_world_rustbuffer_alloc").unwrap();
    let free = std::ffi::CString::new("ffi_hello_world_rustbuffer_free").unwrap();
    let from_bytes = std::ffi::CString::new("ffi_hello_world_rustbuffer_from_bytes").unwrap();
    let describe_name = std::ffi::CString::new("uniffi_hello_world_fn_func_describe").unwrap();
    let u32_tag = std::ffi::CString::new(TAG_UINT32).unwrap();
    let rustbuffer_tag = std::ffi::CString::new(TAG_RUSTBUFFER).unwrap();
    let arg_tag_names = [u32_tag.as_ptr()];
    let fn_spec = UbrnFunctionSpec {
        name: describe_name.as_ptr(),
        arg_tag_names: arg_tag_names.as_ptr(),
        n_args: 1,
        arg_type_names: std::ptr::null(),
        ret_tag_name: rustbuffer_tag.as_ptr(),
        has_rust_call_status: 1,
    };
    let spec = UbrnModuleSpec {
        rustbuffer_alloc: alloc.as_ptr(),
        rustbuffer_free: free.as_ptr(),
        rustbuffer_from_bytes: from_bytes.as_ptr(),
        functions: &fn_spec,
        n_functions: 1,
        callbacks: std::ptr::null(),
        n_callbacks: 0,
        structs: std::ptr::null(),
        n_structs: 0,
    };
    let mut err = [0i8; 256];
    let m = unsafe { ubrn_jsi_register(lib.as_ptr(), &spec, err.as_mut_ptr(), err.len()) };
    assert!(!m.is_null(), "register failed: {:?}", unsafe {
        std::ffi::CStr::from_ptr(err.as_ptr())
    });
    m
}

#[test]
fn describe_returns_rustbuffer() {
    use std::ffi::c_void;
    use std::mem::size_of;
    use uniffi_runtime_core::ffi_c_types::{RustBufferC, RustCallStatusC};

    let describe_name = std::ffi::CString::new("uniffi_hello_world_fn_func_describe").unwrap();

    let m = register_with_describe();

    let n: u32 = 5;
    let arg_ptrs: [*const u8; 1] = [&n as *const u32 as *const u8];
    let arg_sizes: [usize; 1] = [size_of::<u32>()];
    let mut status = RustCallStatusC::default();
    let mut out = [0u8; size_of::<RustBufferC>()];

    let rc = unsafe {
        ubrn_jsi_call(
            m,
            describe_name.as_ptr(),
            arg_ptrs.as_ptr(),
            arg_sizes.as_ptr(),
            1,
            &mut status as *mut RustCallStatusC as *mut c_void,
            out.as_mut_ptr(),
            size_of::<RustBufferC>(),
        )
    };
    assert_eq!(rc, 0, "ubrn_jsi_call returned error code {rc} (6 = unsupported return type; 5 = any other call failure, including a too-small return buffer)");
    assert_eq!(
        status.code, 0,
        "RustCallStatus code was non-zero: {}",
        status.code
    );

    // Reinterpret the 24 output bytes as a RustBufferC.
    let rb: RustBufferC = unsafe { std::ptr::read_unaligned(out.as_ptr() as *const RustBufferC) };
    assert!(!rb.data.is_null(), "RustBuffer data pointer is null");
    assert!(rb.len > 0, "RustBuffer len is 0");

    // Read the bytes and confirm the serialization: a top-level String return is
    // raw UTF-8 with no length prefix — so describe(5) should yield exactly b"n=5".
    let bytes = unsafe { std::slice::from_raw_parts(rb.data, rb.len as usize) };
    assert_eq!(bytes, b"n=5", "unexpected bytes: {:?}", bytes);

    unsafe { ubrn_jsi_rustbuffer_free(m, rb) };
    unsafe { ubrn_jsi_free(m) };
}
