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

/// Player tag name for a u32 scalar; the spec structs carry names, not numbers.
pub const TAG_UINT32: &str = "UInt32";

/// Player tag names from the wire vocabulary in
/// `runtimes/jsi/include/ubrn_jsi.h`, used by the callback roundtrip tests.
pub const TAG_VOID: &str = "Void";
pub const TAG_INT32: &str = "Int32";
pub const TAG_STRUCT: &str = "Struct";
pub const TAG_HANDLE: &str = "Handle";
pub const TAG_CALLBACK: &str = "Callback";

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
    let u32_tag = CString::new(TAG_UINT32).unwrap();
    let tag_names = [u32_tag.as_ptr(), u32_tag.as_ptr()];
    let fn_spec = UbrnFunctionSpec {
        name: add.as_ptr(),
        arg_tag_names: tag_names.as_ptr(),
        n_args: 2,
        arg_type_names: std::ptr::null(),
        ret_tag_name: u32_tag.as_ptr(),
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
    /// A second callback whose declared args include a by-value struct — the
    /// shape of the generated `ForeignFutureComplete*` completers. Core builds
    /// a CIF for it but gives it no flat arg layout.
    pub struct_arg_cb_name: CString,
}

/// Register the `uniffi-fixture-callbacks` fixture with TWO callback methods —
/// `TestCallbackMethod` (`fn(i32) -> void` with a RustCallStatus out-param) and
/// `TestCallbackWithStructArg` (`fn(handle, TestVTable) -> void`, a struct
/// passed BY VALUE) — and ONE vtable struct (`TestVTable`, one Callback field
/// of the first method) plus the three rustbuffer symbols.
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

    let void_tag = CString::new(TAG_VOID).unwrap();

    // A callback method: fn(i32) -> void, with a RustCallStatus out-param.
    let cb_name = CString::new("TestCallbackMethod").unwrap();
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

    // A vtable struct with one field of the callback type above.
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

    // A completer-shaped callback: one of its declared args is the struct
    // BY VALUE. Core builds a CIF for it (libffi knows the struct's layout) but
    // refuses it a flat arg layout — and, for the same reason, a trampoline.
    let struct_arg_cb_name = CString::new("TestCallbackWithStructArg").unwrap();
    let handle_tag = CString::new(TAG_HANDLE).unwrap();
    let struct_tag = CString::new(TAG_STRUCT).unwrap();
    let struct_arg_cb_tag_names = [handle_tag.as_ptr(), struct_tag.as_ptr()];
    let struct_arg_cb_names: [*const std::os::raw::c_char; 2] =
        [std::ptr::null(), struct_name.as_ptr()];
    let struct_arg_cb_spec = UbrnCallbackSpec {
        name: struct_arg_cb_name.as_ptr(),
        arg_tag_names: struct_arg_cb_tag_names.as_ptr(),
        arg_type_names: struct_arg_cb_names.as_ptr(),
        n_args: 2,
        ret_tag_name: void_tag.as_ptr(),
        has_rust_call_status: 0,
        out_return: 0,
        ret_type_name: std::ptr::null(),
    };

    let cb_specs: [UbrnCallbackSpec; 2] = [cb_spec, struct_arg_cb_spec];

    let spec = UbrnModuleSpec {
        rustbuffer_alloc: alloc.as_ptr(),
        rustbuffer_free: free.as_ptr(),
        rustbuffer_from_bytes: from_bytes.as_ptr(),
        functions: std::ptr::null(),
        n_functions: 0,
        callbacks: cb_specs.as_ptr(),
        n_callbacks: cb_specs.len(),
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
        struct_arg_cb_name,
    }
}
