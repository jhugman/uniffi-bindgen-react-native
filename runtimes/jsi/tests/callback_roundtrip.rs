/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Task 3.1: prove the C-ABI parse path for callback + struct definitions.
//!
//! Registers the `callbacks` fixture supplying ONE `UbrnCallbackSpec` (a method
//! signature) and ONE `UbrnStructSpec` (the vtable) plus the rustbuffer symbols,
//! and asserts `ubrn_jsi_register` succeeds. This exercises only the *parsing*
//! of callback/struct defs across the ABI — no trampolines or CIF building for
//! callbacks yet (Tasks 3.2–3.4).

mod common;
use common::{fixture_cdylib_for, register_callbacks_fixture};

use std::ffi::{CStr, CString};

use std::ffi::c_void;

use uniffi_runtime_jsi::{
    ubrn_jsi_free, ubrn_jsi_make_trampoline, ubrn_jsi_register, ubrn_jsi_struct_field_offsets,
    UbrnCallbackSpec, UbrnFunctionSpec, UbrnModuleSpec, UbrnStructField, UbrnStructSpec,
};

// Tags mirrored from runtimes/jsi/include/ubrn_jsi.h.
const UBRN_TY_VOID: u8 = 0;
const UBRN_TY_I32: u8 = 6;
const UBRN_TY_CALLBACK: u8 = 13;
const UBRN_TY_REFERENCE: u8 = 15;

#[test]
fn register_with_callback_and_struct() {
    let fixture = register_callbacks_fixture();
    unsafe { ubrn_jsi_free(fixture.module) };
}

/// `ubrn_jsi_struct_field_offsets` returns a sensible layout for a registered
/// struct, and upholds its null-safety / error contract.
///
/// Registers the same one-Callback-field `TestVTable` struct as above (the
/// Callback field is a pointer-sized fn ptr), then queries the C struct layout
/// the C++ shim uses to marshal a JS object into struct bytes. Reuses the
/// existing registration scaffolding rather than building a new fixture.
#[test]
fn struct_field_offsets_reports_layout_and_handles_errors() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;
    let struct_name = &fixture.struct_name;

    // Query the layout for the real registered struct (1 field, pointer-sized).
    let mut total: usize = 0;
    let mut offsets = [usize::MAX; 4];
    let mut sizes = [0usize; 4];
    let n = unsafe {
        ubrn_jsi_struct_field_offsets(
            m,
            struct_name.as_ptr(),
            &mut total,
            offsets.as_mut_ptr(),
            sizes.as_mut_ptr(),
            offsets.len(),
        )
    };
    assert_eq!(n, 1, "TestVTable has exactly one field");
    assert_eq!(offsets[0], 0, "first field starts at offset 0");
    assert!(
        sizes[0] > 0,
        "the Callback (fn-ptr) field is non-zero-sized"
    );
    assert!(
        total >= offsets[0] + sizes[0],
        "total struct size covers the field (total={total}, off={}, size={})",
        offsets[0],
        sizes[0],
    );

    // Null-safety / error contract: -1 on null name and unknown struct name;
    // null out-pointers are tolerated (return is still the field count).
    let bad_name = CString::new("NoSuchStruct").unwrap();
    assert_eq!(
        unsafe {
            ubrn_jsi_struct_field_offsets(
                m,
                bad_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        },
        -1,
        "unknown struct name => -1",
    );
    assert_eq!(
        unsafe {
            ubrn_jsi_struct_field_offsets(
                m,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        },
        -1,
        "null struct name => -1",
    );
    // Known struct with all-null out-pointers: tolerated, still returns the count.
    assert_eq!(
        unsafe {
            ubrn_jsi_struct_field_offsets(
                m,
                struct_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        },
        1,
        "null out-pointers tolerated; field count still returned",
    );

    // Null module => -1.
    assert_eq!(
        unsafe {
            ubrn_jsi_struct_field_offsets(
                std::ptr::null_mut(),
                struct_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        },
        -1,
        "null module => -1",
    );

    unsafe { ubrn_jsi_free(m) };
}

// Trivial seam fns for the trampoline test. Creating a trampoline only stores
// these in the closure userdata; it does not invoke them, so trivial bodies suffice.
extern "C" fn test_on_js_thread(_args: *const u8, _ret: *mut u8, _user_data: *const c_void) {}

extern "C" fn test_dispatch(
    on_js_thread: extern "C" fn(*const u8, *mut u8, *const c_void),
    args: *const u8,
    ret: *mut u8,
    user_data: *const c_void,
) {
    on_js_thread(args, ret, user_data);
}

extern "C" fn test_is_js_thread(_user_data: *const c_void) -> bool {
    true
}

/// Task 3.2: a registered callback name yields a non-null libffi trampoline fn ptr.
#[test]
fn make_trampoline_returns_non_null() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;
    let cb_name = &fixture.cb_name;

    let fn_ptr = unsafe {
        ubrn_jsi_make_trampoline(
            m,
            cb_name.as_ptr(),
            Some(test_on_js_thread),
            Some(test_dispatch),
            Some(test_is_js_thread),
            std::ptr::null(),
        )
    };
    assert!(!fn_ptr.is_null(), "make_trampoline returned a null fn ptr");

    unsafe { ubrn_jsi_free(m) };
}

/// A function whose single arg is `Reference(Struct(vtable))` (a vtable pointer)
/// builds a valid CIF at register time.
///
/// This is the shape a generated vtable-init function takes: its arg is carried
/// as `UBRN_TY_REFERENCE` (tag 15) — a pointer to the named vtable struct — NOT a
/// bare `UBRN_TY_STRUCT`. A bare `Struct` is rejected as a function-arg slot
/// (`runtimes/core/src/call.rs::slot_size_align`), so a non-null register here
/// proves the Reference(Struct) pointer arg path. The function symbol is a real
/// `init_callback_vtable_*` export of the callbacks fixture.
#[test]
fn register_function_with_reference_struct_arg() {
    let lib = CString::new(fixture_cdylib_for(
        "uniffi-fixture-callbacks",
        "uniffi_fixture_callbacks",
    ))
    .unwrap();
    let alloc = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_alloc").unwrap();
    let free = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_free").unwrap();
    let from_bytes = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_from_bytes").unwrap();

    // The callback method the vtable struct references.
    let cb_name = CString::new("TestCallbackMethod").unwrap();
    let cb_arg_tags: [u8; 1] = [UBRN_TY_I32];
    let cb_spec = UbrnCallbackSpec {
        name: cb_name.as_ptr(),
        arg_tags: cb_arg_tags.as_ptr(),
        arg_type_names: std::ptr::null(),
        n_args: 1,
        ret_tag: UBRN_TY_VOID,
        has_rust_call_status: 1,
        out_return: 0,
        ret_type_name: std::ptr::null(),
    };

    // The vtable struct, with one field of the callback type above.
    let field_name = CString::new("method").unwrap();
    let field_type_name = CString::new("TestCallbackMethod").unwrap();
    let struct_fields: [UbrnStructField; 1] = [UbrnStructField {
        field_name: field_name.as_ptr(),
        type_tag: UBRN_TY_CALLBACK,
        type_name: field_type_name.as_ptr(),
    }];
    let struct_name = CString::new("TestVTable").unwrap();
    let struct_spec = UbrnStructSpec {
        name: struct_name.as_ptr(),
        fields: struct_fields.as_ptr(),
        n_fields: 1,
    };

    // A vtable-init function: fn(Reference(Struct("TestVTable"))) -> void.
    // The arg type name (in the parallel arg_type_names array) is the struct name.
    let fn_name =
        CString::new("uniffi_uniffi_fixture_callbacks_fn_init_callback_vtable_foreigngetters")
            .unwrap();
    let fn_arg_tags: [u8; 1] = [UBRN_TY_REFERENCE];
    let fn_arg_type_names: [*const std::os::raw::c_char; 1] = [struct_name.as_ptr()];
    let fn_spec = UbrnFunctionSpec {
        name: fn_name.as_ptr(),
        arg_tags: fn_arg_tags.as_ptr(),
        n_args: 1,
        arg_type_names: fn_arg_type_names.as_ptr(),
        ret_tag: UBRN_TY_VOID,
        has_rust_call_status: 0,
    };

    let spec = UbrnModuleSpec {
        rustbuffer_alloc: alloc.as_ptr(),
        rustbuffer_free: free.as_ptr(),
        rustbuffer_from_bytes: from_bytes.as_ptr(),
        functions: &fn_spec,
        n_functions: 1,
        callbacks: &cb_spec,
        n_callbacks: 1,
        structs: &struct_spec,
        n_structs: 1,
    };

    let mut err = [0i8; 256];
    let m = unsafe { ubrn_jsi_register(lib.as_ptr(), &spec, err.as_mut_ptr(), err.len()) };
    assert!(!m.is_null(), "register failed: {:?}", unsafe {
        CStr::from_ptr(err.as_ptr())
    });

    unsafe { ubrn_jsi_free(m) };
}
