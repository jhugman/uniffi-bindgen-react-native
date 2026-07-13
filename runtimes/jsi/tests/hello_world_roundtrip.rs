/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! End-to-end test of the C ABI against the real `hello-world` fixture cdylib.
//! Builds the fixture, dlopens it via the C ABI, and calls `add(2, 3)`.

mod common;
use common::{fixture_cdylib, UBRN_TY_U32};

use std::ffi::{c_void, CString};
use std::mem::size_of;

use uniffi_runtime_core::ffi_c_types::RustCallStatusC;
use uniffi_runtime_jsi::{ubrn_jsi_call, ubrn_jsi_free, ubrn_jsi_register};
// The repr(C) spec structs are part of the public surface for the shim; the test
// constructs them directly.
use uniffi_runtime_jsi::{UbrnFunctionSpec, UbrnModuleSpec};

#[test]
fn add_roundtrip() {
    let lib = CString::new(fixture_cdylib()).unwrap();
    let alloc = CString::new("ffi_hello_world_rustbuffer_alloc").unwrap();
    let free = CString::new("ffi_hello_world_rustbuffer_free").unwrap();
    let from_bytes = CString::new("ffi_hello_world_rustbuffer_from_bytes").unwrap();
    let add_name = CString::new("uniffi_hello_world_fn_func_add").unwrap();

    // add(u32, u32) -> u32, with a trailing RustCallStatus.
    let arg_tags: [u8; 2] = [UBRN_TY_U32, UBRN_TY_U32];
    let fn_spec = UbrnFunctionSpec {
        name: add_name.as_ptr(),
        arg_tags: arg_tags.as_ptr(),
        n_args: 2,
        arg_type_names: std::ptr::null(),
        ret_tag: UBRN_TY_U32,
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

    let a: u32 = 2;
    let b: u32 = 3;
    let arg_ptrs: [*const u8; 2] = [&a as *const u32 as *const u8, &b as *const u32 as *const u8];
    let arg_sizes: [usize; 2] = [size_of::<u32>(), size_of::<u32>()];
    let mut status = RustCallStatusC::default();
    let mut out = [0u8; size_of::<u32>()];

    let rc = unsafe {
        ubrn_jsi_call(
            m,
            add_name.as_ptr(),
            arg_ptrs.as_ptr(),
            arg_sizes.as_ptr(),
            2,
            &mut status as *mut RustCallStatusC as *mut c_void,
            out.as_mut_ptr(),
            size_of::<u32>(),
        )
    };
    assert_eq!(rc, 0, "call returned error code {rc}");
    assert_eq!(status.code, 0, "rust call status was non-zero");
    // Native-endian: the player marshals scalars with `to_ne_bytes`/`from_ne_bytes`,
    // valid because the C++ shim and the Rust engine share the same host.
    assert_eq!(u32::from_ne_bytes(out), 5);

    unsafe { ubrn_jsi_free(m) };
}
