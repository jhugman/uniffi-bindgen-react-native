/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! The callback half of the C ABI, driven the way the C++ shim drives it.
//!
//! Registering the `callbacks` fixture with callback and vtable-struct specs;
//! the two layout accessors the shim marshals through
//! (`ubrn_jsi_struct_field_offsets`, `ubrn_jsi_callback_arg_layout`) and their
//! error contracts; building and reusing trampolines; and `ubrn_jsi_disarm`.
//!
//! Everything goes through the `ubrn_jsi_*` exports rather than core's Rust
//! API, so a change that breaks the shim's view of the ABI fails here instead
//! of in a fixture.

mod common;
use common::{fixture_cdylib_for, register_callbacks_fixture, TAG_CALLBACK, TAG_INT32, TAG_VOID};

use std::ffi::{CStr, CString};

use std::ffi::c_void;

use uniffi_runtime_jsi::{
    ubrn_jsi_callback_arg_layout, ubrn_jsi_disarm, ubrn_jsi_free, ubrn_jsi_make_trampoline,
    ubrn_jsi_register, ubrn_jsi_remember_trampoline, ubrn_jsi_struct_field_offsets,
    ubrn_jsi_trampoline_for, UbrnCallbackSpec, UbrnFunctionSpec, UbrnModuleSpec, UbrnStructField,
    UbrnStructSpec,
};

/// The one tag name `common` has no use for; the rest come from there.
const TAG_REFERENCE: &str = "Reference";

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

/// `ubrn_jsi_callback_arg_layout` reports the slots in CIF order, and upholds
/// its truncation / null-safety / error contract.
///
/// The fixture callback is `TestCallbackMethod: fn(i32) -> void` with a
/// RustCallStatus out-param, so the buffer core's trampoline packs is
/// [i32 @0 (4 bytes), *mut RustCallStatus @8 (8 bytes)] = 16 bytes, the status
/// pointer realigned to 8. This export is the shim's only source for that
/// arithmetic.
#[test]
fn callback_arg_layout_reports_cif_order_and_handles_errors() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;
    let cb_name = &fixture.cb_name;

    let mut total: usize = 0;
    let mut offsets = [usize::MAX; 4];
    let mut sizes = [usize::MAX; 4];
    let n = unsafe {
        ubrn_jsi_callback_arg_layout(
            m,
            cb_name.as_ptr(),
            &mut total,
            offsets.as_mut_ptr(),
            sizes.as_mut_ptr(),
            offsets.len(),
        )
    };
    assert_eq!(n, 2, "one declared arg plus the RustCallStatus pointer");
    assert_eq!((offsets[0], sizes[0]), (0, 4), "i32 arg @0");
    assert_eq!(
        (offsets[1], sizes[1]),
        (8, 8),
        "RustCallStatus pointer realigned to 8",
    );
    assert_eq!(total, 16, "whole arg buffer is 16 bytes");
    assert_eq!(
        offsets[2],
        usize::MAX,
        "nothing written past the slot count"
    );

    // Truncation: cap smaller than the slot count writes `cap` entries and
    // still returns the real count.
    let mut short = [usize::MAX; 2];
    assert_eq!(
        unsafe {
            ubrn_jsi_callback_arg_layout(
                m,
                cb_name.as_ptr(),
                std::ptr::null_mut(),
                short.as_mut_ptr(),
                std::ptr::null_mut(),
                1,
            )
        },
        2,
        "truncated call still reports the real slot count",
    );
    assert_eq!(short[0], 0, "first slot written");
    assert_eq!(short[1], usize::MAX, "second slot beyond cap, left alone");

    // Error contract: -1 on unknown name, null name, and null module.
    let bad_name = CString::new("NoSuchCallback").unwrap();
    for (name, why) in [
        (bad_name.as_ptr(), "unknown callback name => -1"),
        (std::ptr::null(), "null callback name => -1"),
    ] {
        assert_eq!(
            unsafe {
                ubrn_jsi_callback_arg_layout(
                    m,
                    name,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            },
            -1,
            "{why}",
        );
    }
    assert_eq!(
        unsafe {
            ubrn_jsi_callback_arg_layout(
                std::ptr::null_mut(),
                cb_name.as_ptr(),
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

/// The invariant the shim's slotless-shape tolerance rests on: a callback core
/// gives NO flat arg layout for also gets NO trampoline.
///
/// `buildShape` (cpp/jsi-player-shim/callbacks.cpp) keeps a shape with empty
/// slots when `ubrn_jsi_callback_arg_layout` returns -1 — the generated
/// `ForeignFutureComplete*` completers take their result struct by value, and
/// core will not lay that out in a flat buffer. That empty shape is only safe
/// because the slots are read exclusively on the trampoline path, which core
/// refuses to create for exactly the same signatures — `make_callback_trampoline`
/// takes its layout from the same `callback_arg_layout` accessor. Pin BOTH
/// halves here through the C ABI: if a future change ever lets one succeed
/// while the other refuses, this fails instead of the shim reading past the end
/// of `argSlots`.
#[test]
fn no_arg_layout_implies_no_trampoline() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;
    let cb_name = &fixture.struct_arg_cb_name;

    let mut total = usize::MAX;
    let n = unsafe {
        ubrn_jsi_callback_arg_layout(
            m,
            cb_name.as_ptr(),
            &mut total,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    // Not an unknown-name -1: registration inserts every spec'd callback under
    // its name or fails outright, and the module registered, so the key exists.
    assert_eq!(n, -1, "a by-value struct arg has no flat slot layout");
    assert_eq!(total, usize::MAX, "nothing written on the error path");

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
    assert!(
        fn_ptr.is_null(),
        "core refused the layout but built a trampoline: the shim's empty-slot \
         shape would now be reachable, and its slot reads out of bounds",
    );

    // The sibling callback, which core DOES lay out, still gets both.
    assert!(
        unsafe {
            ubrn_jsi_callback_arg_layout(
                m,
                fixture.cb_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        } > 0,
        "a layable callback still reports its slots",
    );

    unsafe { ubrn_jsi_free(m) };
}

/// A registered callback name yields a non-null libffi trampoline fn ptr.
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

/// The reuse map the shim marshals callbacks through: a miss, a record, a hit,
/// and the two ways a key can differ.
///
/// The shim mints one identity per JS function and remembers a trampoline under
/// (callback name, identity). Reuse being keyed on the PAIR is what keeps two
/// different functions — or one function used for two callbacks — off each
/// other's trampoline, which no fixture can observe: a wrong hit still calls
/// into JS, just the wrong JS. Pin it here, through the C ABI the shim uses.
#[test]
fn trampoline_reuse_is_keyed_on_name_and_identity() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;
    let cb_name = &fixture.cb_name;
    let other_name = &fixture.struct_arg_cb_name;
    let fn_ptr = 0x1234 as *const c_void;

    assert!(
        unsafe { ubrn_jsi_trampoline_for(m, cb_name.as_ptr(), 1) }.is_null(),
        "nothing remembered yet",
    );
    unsafe { ubrn_jsi_remember_trampoline(m, cb_name.as_ptr(), 1, fn_ptr) };
    assert_eq!(
        unsafe { ubrn_jsi_trampoline_for(m, cb_name.as_ptr(), 1) },
        fn_ptr,
        "the same pair reuses the trampoline",
    );
    assert!(
        unsafe { ubrn_jsi_trampoline_for(m, cb_name.as_ptr(), 2) }.is_null(),
        "a second JS function must not inherit the first's trampoline",
    );
    assert!(
        unsafe { ubrn_jsi_trampoline_for(m, other_name.as_ptr(), 1) }.is_null(),
        "one JS function under a second callback name needs its own trampoline",
    );

    // Null module / null name are a miss, not a crash: the shim treats both as
    // "build one", so a lookup and a record must tolerate them symmetrically.
    assert!(
        unsafe { ubrn_jsi_trampoline_for(std::ptr::null_mut(), cb_name.as_ptr(), 1) }.is_null(),
    );
    assert!(unsafe { ubrn_jsi_trampoline_for(m, std::ptr::null(), 1) }.is_null());
    unsafe { ubrn_jsi_remember_trampoline(std::ptr::null_mut(), cb_name.as_ptr(), 3, fn_ptr) };
    unsafe { ubrn_jsi_remember_trampoline(m, std::ptr::null(), 3, fn_ptr) };
    assert!(
        unsafe { ubrn_jsi_trampoline_for(m, cb_name.as_ptr(), 3) }.is_null(),
        "a rejected record leaves the map untouched",
    );

    unsafe { ubrn_jsi_free(m) };
}

/// A function whose single arg is `Reference(Struct(vtable))` (a vtable pointer)
/// builds a valid CIF at register time.
///
/// This is the shape a generated vtable-init function takes: its arg is carried
/// as `"Reference"` — a pointer to the named vtable struct — NOT a bare
/// `"Struct"`. A bare `Struct` is rejected as a function-arg slot
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
    let void_tag = CString::new(TAG_VOID).unwrap();
    let i32_tag = CString::new(TAG_INT32).unwrap();
    let cb_arg_tag_names = [i32_tag.as_ptr()];
    let cb_spec = UbrnCallbackSpec {
        name: cb_name.as_ptr(),
        arg_tag_names: cb_arg_tag_names.as_ptr(),
        arg_type_names: std::ptr::null(),
        n_args: 1,
        ret_tag_name: void_tag.as_ptr(),
        has_rust_call_status: 1,
        out_return: 0,
        ret_type_name: std::ptr::null(),
    };

    // The vtable struct, with one field of the callback type above.
    let field_name = CString::new("method").unwrap();
    let field_type_name = CString::new("TestCallbackMethod").unwrap();
    let callback_tag = CString::new(TAG_CALLBACK).unwrap();
    let struct_fields: [UbrnStructField; 1] = [UbrnStructField {
        field_name: field_name.as_ptr(),
        type_tag_name: callback_tag.as_ptr(),
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
    let reference_tag = CString::new(TAG_REFERENCE).unwrap();
    let fn_arg_tag_names = [reference_tag.as_ptr()];
    let fn_arg_type_names: [*const std::os::raw::c_char; 1] = [struct_name.as_ptr()];
    let fn_spec = UbrnFunctionSpec {
        name: fn_name.as_ptr(),
        arg_tag_names: fn_arg_tag_names.as_ptr(),
        n_args: 1,
        arg_type_names: fn_arg_type_names.as_ptr(),
        ret_tag_name: void_tag.as_ptr(),
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

/// The shim's only way to reach the unloading flag, and the whole of what a
/// runtime teardown asks of core. After it, the module stays valid to call and
/// every trampoline it handed out is inert — but nothing new is built.
#[test]
fn disarm_stops_anything_further_being_built() {
    let fixture = register_callbacks_fixture();
    let m = fixture.module;

    let before = unsafe {
        ubrn_jsi_make_trampoline(
            m,
            fixture.cb_name.as_ptr(),
            Some(test_on_js_thread),
            Some(test_dispatch),
            Some(test_is_js_thread),
            std::ptr::null(),
        )
    };
    assert!(!before.is_null(), "an armed module builds a trampoline");

    // Null is tolerated and a second disarm is a no-op: a reload can reach this
    // twice, and a module can be registered against a runtime that never opened.
    unsafe { ubrn_jsi_disarm(std::ptr::null_mut()) };
    unsafe { ubrn_jsi_disarm(m) };
    unsafe { ubrn_jsi_disarm(m) };

    let after = unsafe {
        ubrn_jsi_make_trampoline(
            m,
            fixture.cb_name.as_ptr(),
            Some(test_on_js_thread),
            Some(test_dispatch),
            Some(test_is_js_thread),
            std::ptr::null(),
        )
    };
    assert!(
        after.is_null(),
        "a disarmed module built another trampoline: it can never be called, \
         so it is a permanent leak",
    );

    // The module is still a live handle — disarm frees nothing.
    unsafe { ubrn_jsi_free(m) };
}
