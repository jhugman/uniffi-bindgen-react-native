/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Shared test helpers: fixture cdylib path and module registration boilerplate.

use std::ffi::CString;
use std::process::Command;

use uniffi_runtime_jsi::{
    ubrn_jsi_register, UbrnCallbackSpec, UbrnFunctionSpec, UbrnJsiModule, UbrnModuleSpec,
    UbrnStructField, UbrnStructSpec,
};

/// Type tag for a u32 scalar; mirrors `UBRN_TY_U32` in `runtimes/jsi/include/ubrn_jsi.h`.
pub const UBRN_TY_U32: u8 = 5;

/// Type tags mirrored from `runtimes/jsi/include/ubrn_jsi.h`, used by the callback
/// roundtrip tests.
pub const UBRN_TY_VOID: u8 = 0;
pub const UBRN_TY_I32: u8 = 6;
pub const UBRN_TY_CALLBACK: u8 = 13;

/// Build the `hello-world` fixture cdylib and return its path.
///
/// The build is cached by Cargo — repeated calls within the same test run are fast.
pub fn fixture_cdylib() -> String {
    fixture_cdylib_for("uniffi-fixture-hello-world", "hello_world")
}

/// Build the cdylib for fixture package `pkg` (the Cargo package name) and return
/// the path to its shared library, where `libname` is the `[lib] name` (the dylib
/// is `lib<libname>.<ext>` on unix, `<libname>.dll` on Windows).
///
/// The build is cached by Cargo — repeated calls within the same test run are fast.
#[allow(dead_code)]
pub fn fixture_cdylib_for(pkg: &str, libname: &str) -> String {
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", pkg, "--lib"])
        .status()
        .expect("cargo build");
    assert!(status.success(), "fixture build failed for {pkg}");
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let prefix = if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    };
    let root = format!("{}/../../target/debug", env!("CARGO_MANIFEST_DIR"));
    format!("{root}/{prefix}{libname}.{ext}")
}

/// Register the `hello-world` fixture with `add` + the three rustbuffer symbols.
///
/// The returned pointer must be freed with [`ubrn_jsi_free`].
#[allow(dead_code)]
pub fn register_hello_world() -> *mut UbrnJsiModule {
    let lib = CString::new(fixture_cdylib()).unwrap();
    let alloc = CString::new("ffi_hello_world_rustbuffer_alloc").unwrap();
    let free = CString::new("ffi_hello_world_rustbuffer_free").unwrap();
    let from_bytes = CString::new("ffi_hello_world_rustbuffer_from_bytes").unwrap();
    let add = CString::new("uniffi_hello_world_fn_func_add").unwrap();
    let tags = [UBRN_TY_U32, UBRN_TY_U32];
    let fn_spec = UbrnFunctionSpec {
        name: add.as_ptr(),
        arg_tags: tags.as_ptr(),
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
    m
}

/// What [`register_callbacks_fixture`] returns: the registered module plus the
/// `cb_name`/`struct_name` CStrings, which must outlive the module for tests that
/// re-use their pointers after registration (e.g. `make_trampoline`,
/// `struct_field_offsets`).
#[allow(dead_code)]
pub struct CallbacksFixture {
    pub module: *mut UbrnJsiModule,
    pub cb_name: CString,
    pub struct_name: CString,
}

/// Register the `uniffi-fixture-callbacks` fixture with ONE callback method
/// (`TestCallbackMethod`: `fn(i32) -> void` with a RustCallStatus out-param) and
/// ONE vtable struct (`TestVTable`, one Callback field of that method) plus the
/// three rustbuffer symbols.
///
/// Mirrors [`register_hello_world`] but for the callback/struct parse path. The
/// returned module must be freed with [`ubrn_jsi_free`]; the returned CStrings
/// back pointers the C ABI copied, so callers that keep `cb_name`/`struct_name`
/// pointers (via the original CStrings) get them in [`CallbacksFixture`].
#[allow(dead_code)]
pub fn register_callbacks_fixture() -> CallbacksFixture {
    let lib = CString::new(fixture_cdylib_for(
        "uniffi-fixture-callbacks",
        "uniffi_fixture_callbacks",
    ))
    .unwrap();
    let alloc = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_alloc").unwrap();
    let free = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_free").unwrap();
    let from_bytes = CString::new("ffi_uniffi_fixture_callbacks_rustbuffer_from_bytes").unwrap();

    // A callback method: fn(i32) -> void, with a RustCallStatus out-param.
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

    // A vtable struct with one field of the callback type above.
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

    let spec = UbrnModuleSpec {
        rustbuffer_alloc: alloc.as_ptr(),
        rustbuffer_free: free.as_ptr(),
        rustbuffer_from_bytes: from_bytes.as_ptr(),
        functions: std::ptr::null(),
        n_functions: 0,
        callbacks: &cb_spec,
        n_callbacks: 1,
        structs: &struct_spec,
        n_structs: 1,
    };

    let mut err = [0i8; 256];
    let module = unsafe { ubrn_jsi_register(lib.as_ptr(), &spec, err.as_mut_ptr(), err.len()) };
    assert!(!module.is_null(), "register failed: {:?}", unsafe {
        std::ffi::CStr::from_ptr(err.as_ptr())
    });

    CallbacksFixture {
        module,
        cb_name,
        struct_name,
    }
}
