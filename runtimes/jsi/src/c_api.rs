/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Hand-written extern "C" surface over `uniffi-runtime-core`.
//!
//! This is the authoritative Rust side of the ABI declared in
//! `include/ubrn_jsi.h`. A React Native C++ JSI shim dlopens this library (or
//! links it statically) and calls `ubrn_jsi_register` / `ubrn_jsi_call` /
//! `ubrn_jsi_free`. Scalars, RustBuffer (both as arguments and return values),
//! and a `RustCallStatus` out-parameter are supported. Callback and struct
//! *definitions* are parsed into the `ModuleSpec` here; callback trampoline
//! creation, vtable building, and fn-pointer invocation are exposed via
//! `ubrn_jsi_make_trampoline` / `ubrn_jsi_make_vtable`. Async functions and
//! async callbacks are supported via `ubrn_jsi_set_dispatch`. Distribution
//! packaging and gen_cpp cutover are out of scope for this runtime layer.

use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::path::Path;
use std::sync::Arc;

use uniffi_runtime_core::slot;
use uniffi_runtime_core::{
    CallReturn, CallbackDef, DispatchFn, FfiTypeDesc, FunctionDef, IsJsThreadFn, Module,
    ModuleSpec, OnJsThreadFn, RustBufferSymbols, StructDef, StructField, VTableField,
};

/// Build a slice from a caller-supplied array pointer + length.
///
/// `std::slice::from_raw_parts` requires a non-null, aligned pointer even when
/// `len == 0`, but a C++ caller may legitimately pass NULL for an empty array.
/// Yield an empty slice in that case without dereferencing the pointer.
///
/// # Safety
/// `ptr` must be valid for `len` elements, or null when `len == 0`.
unsafe fn slice_or_empty<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len)
    }
}

/// Map a pure-scalar C ABI type tag (see ubrn_jsi.h `UbrnFfiType`) to a core
/// `FfiTypeDesc`. Returns `None` for the named tags (`Callback`/`Struct`) and any
/// unknown tag; use [`ffi_type_from_tag_named`] when a type name may be present.
fn ffi_type_from_tag(tag: u8) -> Option<FfiTypeDesc> {
    Some(match tag {
        0 => FfiTypeDesc::Void,
        1 => FfiTypeDesc::UInt8,
        2 => FfiTypeDesc::Int8,
        3 => FfiTypeDesc::UInt16,
        4 => FfiTypeDesc::Int16,
        5 => FfiTypeDesc::UInt32,
        6 => FfiTypeDesc::Int32,
        7 => FfiTypeDesc::UInt64,
        8 => FfiTypeDesc::Int64,
        9 => FfiTypeDesc::Float32,
        10 => FfiTypeDesc::Float64,
        11 => FfiTypeDesc::Handle,
        12 => FfiTypeDesc::RustBuffer,
        16 => FfiTypeDesc::RustCallStatus,
        _ => return None,
    })
}

/// Map a C ABI type tag plus an optional type name to a core `FfiTypeDesc`.
///
/// Tag 13 (`Callback`), tag 14 (`Struct`), and tag 15 (`Reference`) carry a
/// `name` (from the parallel `*_type_names` array); they error if `name` is
/// `None`. Tag 15 always wraps a named `Struct` (a callback-interface vtable
/// pointer), mapping to `Reference(Struct(name))` — a pointer-sized arg slot
/// (a bare `Struct` is not a supported function-arg slot). All other tags defer
/// to [`ffi_type_from_tag`] and ignore `name`. Returns `Err` on an unknown tag
/// or a named tag with a missing name.
fn ffi_type_from_tag_named(tag: u8, name: Option<&str>) -> Result<FfiTypeDesc, String> {
    match tag {
        13 => name
            .map(|n| FfiTypeDesc::Callback(n.to_owned()))
            .ok_or_else(|| "Callback tag (13) requires a type name".to_owned()),
        14 => name
            .map(|n| FfiTypeDesc::Struct(n.to_owned()))
            .ok_or_else(|| "Struct tag (14) requires a type name".to_owned()),
        15 => name
            .map(|n| FfiTypeDesc::Reference(Box::new(FfiTypeDesc::Struct(n.to_owned()))))
            .ok_or_else(|| "Reference tag (15) requires a struct type name".to_owned()),
        _ => ffi_type_from_tag(tag).ok_or_else(|| format!("bad type tag {tag}")),
    }
}

/// Parse a parallel `(arg_tags, arg_type_names)` pair of length `n` into a vector
/// of `FfiTypeDesc`.
///
/// `arg_type_names` is parallel to `arg_tags`: it supplies the name for Callback
/// (tag 13) and Struct (tag 14) args and is ignored for scalars. A null
/// `arg_type_names` array is treated as "all names absent" (every name `None`).
///
/// # Safety
/// `arg_tags` must be valid for `n` elements (or null when `n == 0`).
/// `arg_type_names`, when non-null, must be valid for `n` elements.
unsafe fn parse_args(
    arg_tags: *const u8,
    arg_type_names: *const *const c_char,
    n: usize,
) -> Result<Vec<FfiTypeDesc>, String> {
    let tags = slice_or_empty(arg_tags, n);
    // Treat a null names array as all-None; otherwise borrow `n` name pointers.
    let names: &[*const c_char] = if arg_type_names.is_null() {
        &[]
    } else {
        slice_or_empty(arg_type_names, n)
    };
    let mut args = Vec::with_capacity(n);
    for (i, &t) in tags.iter().enumerate() {
        let name = names.get(i).copied().and_then(|p| cstr(p));
        args.push(ffi_type_from_tag_named(t, name.as_deref())?);
    }
    Ok(args)
}

/// Opaque handle handed back to the C++ shim. Wraps the core `Module`, which
/// already retains the resolved `FunctionDef`s (reachable via the `Module` API)
/// for any later phase that needs per-function layout.
pub struct UbrnJsiModule {
    module: Arc<Module>,
}

#[repr(C)]
pub struct UbrnFunctionSpec {
    pub name: *const c_char,
    pub arg_tags: *const u8,
    pub n_args: usize,
    /// Parallel to `arg_tags` (length `n_args`): type name for Callback/Struct
    /// tags, null for scalars. The whole array may be null. Appended AFTER
    /// `n_args`, matching `ubrn_jsi.h`'s `UbrnFunctionSpec`.
    pub arg_type_names: *const *const c_char,
    pub ret_tag: u8,
    pub has_rust_call_status: u8,
}

#[repr(C)]
pub struct UbrnCallbackSpec {
    pub name: *const c_char,
    pub arg_tags: *const u8,
    pub arg_type_names: *const *const c_char,
    pub n_args: usize,
    pub ret_tag: u8,
    pub has_rust_call_status: u8,
    pub out_return: u8,
    /// Type name for a Struct (tag 14) return, else null. Carries the struct name
    /// so a Struct-returning callback parses to `Struct(name)` instead of erroring;
    /// core ignores it when `out_return` is set. Mirrors `ubrn_jsi.h`'s field order
    /// (appended after `out_return`).
    pub ret_type_name: *const c_char,
}

#[repr(C)]
pub struct UbrnStructField {
    pub field_name: *const c_char,
    pub type_tag: u8,
    pub type_name: *const c_char,
}

#[repr(C)]
pub struct UbrnStructSpec {
    pub name: *const c_char,
    pub fields: *const UbrnStructField,
    pub n_fields: usize,
}

#[repr(C)]
pub struct UbrnModuleSpec {
    pub rustbuffer_alloc: *const c_char,
    pub rustbuffer_free: *const c_char,
    pub rustbuffer_from_bytes: *const c_char,
    pub functions: *const UbrnFunctionSpec,
    pub n_functions: usize,
    pub callbacks: *const UbrnCallbackSpec,
    pub n_callbacks: usize,
    pub structs: *const UbrnStructSpec,
    pub n_structs: usize,
}

/// The abort callback core requires. The player shim has no engine-specific
/// callback resources to abort, so this is a no-op.
extern "C" fn noop_abort(_user_data: *const c_void) {}

/// Read a NUL-terminated C string into an owned `String`.
///
/// # Safety
/// `s` must be a NUL-terminated C string from the shim, or null.
unsafe fn cstr(s: *const c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    CStr::from_ptr(s).to_str().ok().map(|s| s.to_owned())
}

/// Write a message into the caller's error buffer (NUL-terminated, truncated).
///
/// # Safety
/// `buf` must be null or point to at least `len` writable bytes.
unsafe fn write_err(buf: *mut c_char, len: usize, msg: &str) {
    if buf.is_null() || len == 0 {
        return;
    }
    let bytes = msg.as_bytes();
    let n = bytes.len().min(len - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n);
    *buf.add(n) = 0;
}

/// dlopen `lib_path`, resolve symbols, and build CIFs.
///
/// Returns a heap-allocated [`UbrnJsiModule`] on success, or null on error (with
/// a message written into `err_buf`).
///
/// # Safety
/// `lib_path` and `spec` must be valid pointers; the arrays referenced by `spec`
/// (`functions`/`arg_tags`) must have the declared lengths. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_register(
    lib_path: *const c_char,
    spec: *const UbrnModuleSpec,
    err_buf: *mut c_char,
    err_len: usize,
) -> *mut UbrnJsiModule {
    let result = (|| -> Result<*mut UbrnJsiModule, String> {
        let lib_path = cstr(lib_path).ok_or("lib_path is null or not UTF-8")?;
        let spec = spec.as_ref().ok_or("spec is null")?;

        let rustbuffer_symbols = RustBufferSymbols {
            alloc: cstr(spec.rustbuffer_alloc).ok_or("rustbuffer_alloc missing")?,
            free: cstr(spec.rustbuffer_free).ok_or("rustbuffer_free missing")?,
            from_bytes: cstr(spec.rustbuffer_from_bytes).ok_or("rustbuffer_from_bytes missing")?,
        };

        let mut functions = HashMap::new();
        let fn_specs = slice_or_empty(spec.functions, spec.n_functions);
        for f in fn_specs {
            let name = cstr(f.name).ok_or("function name missing")?;
            let args = parse_args(f.arg_tags, f.arg_type_names, f.n_args)?;
            let ret = ffi_type_from_tag_named(f.ret_tag, None)
                .map_err(|e| format!("function {name} return: {e}"))?;
            functions.insert(
                name,
                FunctionDef {
                    args,
                    ret,
                    has_rust_call_status: f.has_rust_call_status != 0,
                },
            );
        }

        let mut callbacks = HashMap::new();
        let cb_specs = slice_or_empty(spec.callbacks, spec.n_callbacks);
        for c in cb_specs {
            let name = cstr(c.name).ok_or("callback name missing")?;
            let args = parse_args(c.arg_tags, c.arg_type_names, c.n_args)?;
            // A callback's return may be a named Struct (written through the
            // out_return pointer); pass ret_type_name so it parses to Struct(name).
            let ret_name = cstr(c.ret_type_name);
            let ret = ffi_type_from_tag_named(c.ret_tag, ret_name.as_deref())
                .map_err(|e| format!("callback {name} return: {e}"))?;
            callbacks.insert(
                name,
                CallbackDef {
                    args,
                    ret,
                    has_rust_call_status: c.has_rust_call_status != 0,
                    out_return: c.out_return != 0,
                },
            );
        }

        let mut structs = HashMap::new();
        let struct_specs = slice_or_empty(spec.structs, spec.n_structs);
        for s in struct_specs {
            let name = cstr(s.name).ok_or("struct name missing")?;
            let field_specs = slice_or_empty(s.fields, s.n_fields);
            let mut fields = Vec::with_capacity(s.n_fields);
            for field in field_specs {
                let field_name = cstr(field.field_name).ok_or("struct field name missing")?;
                let type_name = cstr(field.type_name);
                let field_type = ffi_type_from_tag_named(field.type_tag, type_name.as_deref())
                    .map_err(|e| format!("struct {name} field {field_name}: {e}"))?;
                fields.push(StructField {
                    name: field_name,
                    field_type,
                });
            }
            structs.insert(name, StructDef { fields });
        }

        let module_spec = ModuleSpec {
            rustbuffer_symbols,
            functions,
            callbacks,
            structs,
        };

        let module = Module::new(
            Path::new(&lib_path),
            module_spec,
            noop_abort,
            std::ptr::null(),
        )
        .map_err(|e| format!("Module::new failed: {e}"))?;

        Ok(Box::into_raw(Box::new(UbrnJsiModule { module })))
    })();

    match result {
        Ok(ptr) => ptr,
        Err(msg) => {
            write_err(err_buf, err_len, &msg);
            std::ptr::null_mut()
        }
    }
}

/// Allocate a Rust-owned buffer of `size` bytes via the library's `rustbuffer_alloc`.
///
/// Returns a [`RustBufferC`] with `capacity >= size` and `len == 0` on success, or a
/// zeroed buffer if `m` is null, `size` exceeds `i32::MAX`, or the underlying call fails.
/// The caller is responsible for eventually freeing the buffer with
/// [`ubrn_jsi_rustbuffer_free`].
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`].
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_alloc(
    m: *mut UbrnJsiModule,
    size: u64,
) -> uniffi_runtime_core::ffi_c_types::RustBufferC {
    use uniffi_runtime_core::ffi_c_types::{RustBufferAllocFn, RustBufferC, RustCallStatusC};
    let Some(m) = m.as_ref() else {
        return RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        };
    };
    if size > i32::MAX as u64 {
        return RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        };
    }
    // SAFETY: alloc_ptr was resolved via dlsym at registration; size fits i32 for uniffi buffers.
    let func: RustBufferAllocFn = std::mem::transmute(m.module.rb_ops().alloc_ptr);
    let mut status = RustCallStatusC::default();
    let rb = func(size as i32, &mut status);
    if status.code != 0 {
        return RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        };
    }
    rb
}

/// Copy `len` borrowed bytes into a new Rust-owned buffer via `rustbuffer_from_bytes`.
///
/// Returns a [`RustBufferC`] whose `len == len` on success, or a zeroed buffer on error.
/// The caller is responsible for eventually freeing the buffer with
/// [`ubrn_jsi_rustbuffer_free`].
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]. `data` must be valid for `len`
/// bytes (or null when `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_from_bytes(
    m: *mut UbrnJsiModule,
    data: *const u8,
    len: usize,
) -> uniffi_runtime_core::ffi_c_types::RustBufferC {
    use uniffi_runtime_core::ffi_c_types::RustBufferC;
    let Some(m) = m.as_ref() else {
        return RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        };
    };
    match m.module.rustbuffer_from_bytes(data, len) {
        Ok(rb) => rb,
        Err(_) => RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        },
    }
}

/// Free a Rust-owned buffer via the library's `rustbuffer_free`.
///
/// Null `m` or a zeroed `buf` are silently ignored.
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]. `buf` must have been allocated
/// by the same module (via [`ubrn_jsi_rustbuffer_alloc`] or [`ubrn_jsi_rustbuffer_from_bytes`]).
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_free(
    m: *mut UbrnJsiModule,
    buf: uniffi_runtime_core::ffi_c_types::RustBufferC,
) {
    if let Some(m) = m.as_ref() {
        let _ = m.module.rustbuffer_free(buf);
    }
}

/// Free a module returned by [`ubrn_jsi_register`].
///
/// # Safety
/// `m` must be a pointer returned by `ubrn_jsi_register`, called at most once.
/// Null is tolerated (no-op).
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_free(m: *mut UbrnJsiModule) {
    if !m.is_null() {
        drop(Box::from_raw(m));
    }
}

/// Write a `CallReturn` into `out` as its native-endian byte representation.
///
/// Scalars are written as native-endian bytes. A `RustBuffer` return is written
/// as the 24-byte `repr(C)` layout of [`uniffi_runtime_core::ffi_c_types::RustBufferC`]
/// (`capacity: u64`, `len: u64`, `data: *mut u8`); the caller is responsible for
/// eventually freeing that buffer.
///
/// Returns the number of bytes written, or `None` if `out` is too small.
///
/// **Leak on undersized `out`:** if the return type is `RustBuffer` and `out` is
/// smaller than `size_of::<RustBufferC>()` (24 bytes), this returns `None` and
/// the buffer's heap-allocated backing memory is leaked (no `Drop` on
/// [`CallReturn`]).  The caller MUST size `out` to at least 24 bytes for any
/// RustBuffer-returning function.
fn write_return(ret: &CallReturn, out: &mut [u8]) -> Option<usize> {
    macro_rules! put {
        ($v:expr) => {{
            let b = $v.to_ne_bytes();
            if out.len() < b.len() {
                return None;
            }
            out[..b.len()].copy_from_slice(&b);
            Some(b.len())
        }};
    }
    match ret {
        CallReturn::Void => Some(0),
        CallReturn::U8(v) => put!(v),
        CallReturn::I8(v) => put!(v),
        CallReturn::U16(v) => put!(v),
        CallReturn::I16(v) => put!(v),
        CallReturn::U32(v) => put!(v),
        CallReturn::I32(v) => put!(v),
        CallReturn::U64(v) => put!(v),
        CallReturn::I64(v) => put!(v),
        CallReturn::F32(v) => put!(v),
        CallReturn::F64(v) => put!(v),
        CallReturn::Pointer(v) => put!((*v as u64)),
        CallReturn::RustBuffer(rb) => {
            let bytes = slot::rust_buffer_to_bytes(rb);
            if out.len() < bytes.len() {
                return None;
            }
            out[..bytes.len()].copy_from_slice(&bytes);
            Some(bytes.len())
        }
    }
}

/// Invoke a registered function.
///
/// `args[i]` points to `arg_sizes[i]` native-endian bytes for argument `i`
/// (scalars, RustBuffer, callback fn-ptrs, and struct/vtable pointers). `status`
/// is either null or a pointer to a caller-allocated `RustCallStatus` (layout:
/// [`uniffi_runtime_core::ffi_c_types::RustCallStatusC`]). The native return
/// value is written into `out_ret` (`out_ret_size` bytes).
///
/// Returns 0 on success; non-zero on error:
/// - 1: module handle is null
/// - 2: `fn_name` is null or not UTF-8
/// - 3: function not registered / `prepare_call` failed
/// - 4: argument index out of range
/// - 5: the underlying FFI call failed (e.g. module unloading)
/// - 6: the return value could not be written (unsupported type or buffer too small)
///
/// # Safety
/// All pointers must be valid for the declared lengths; `status` is either null
/// or a pointer to a caller-allocated `RustCallStatus` that stays alive for the
/// duration of the call. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_call(
    m: *mut UbrnJsiModule,
    fn_name: *const c_char,
    args: *const *const u8,
    arg_sizes: *const usize,
    n_args: usize,
    status: *mut c_void,
    out_ret: *mut u8,
    out_ret_size: usize,
) -> i32 {
    let Some(m) = m.as_ref() else { return 1 };
    let Some(name) = cstr(fn_name) else { return 2 };

    let mut call = match m.module.prepare_call(&name) {
        Ok(c) => c,
        Err(_) => return 3,
    };

    let arg_ptrs = slice_or_empty(args, n_args);
    let arg_lens = slice_or_empty(arg_sizes, n_args);
    for i in 0..n_args {
        let Ok(dst) = call.arg_slot(i) else { return 4 };
        let copy_len = arg_lens[i].min(dst.len());
        let src = std::slice::from_raw_parts(arg_ptrs[i], copy_len);
        dst[..src.len()].copy_from_slice(src);
    }

    if !status.is_null() {
        if let Some(rcs_slot) = call.rust_call_status_slot() {
            slot::write_pointer(rcs_slot, status as *const c_void);
        }
    }

    let ret = match m.module.call(call) {
        Ok(r) => r,
        Err(_) => return 5,
    };

    let out: &mut [u8] = if out_ret.is_null() || out_ret_size == 0 {
        &mut []
    } else {
        std::slice::from_raw_parts_mut(out_ret, out_ret_size)
    };
    match write_return(&ret, out) {
        Some(_) => 0,
        None => 6,
    }
}

/// Create a libffi trampoline the loaded Rust library can invoke to call into JS.
///
/// The three function pointers are core's seam types (layout-identical to the
/// `Ubrn{OnJsThread,Dispatch,IsJsThread}Fn` C typedefs), accepted here as
/// `Option<...>` so a null fn ptr from C is rejected rather than miscompiled
/// (`extern "C" fn` is non-nullable in Rust, but `Option<extern "C" fn>` shares
/// its ABI). The trampoline stores the fns plus `user_data` in leaked closure
/// userdata; it is not invoked here.
///
/// Returns the trampoline fn pointer on success, or null on error (null module,
/// any null seam fn, `callback_name` null/not-UTF-8, or unknown callback name).
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]. `callback_name` must be
/// a NUL-terminated C string. The seam fn pointers, when non-null, must point to
/// functions matching the corresponding C typedef. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_make_trampoline(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    on_js_thread: Option<OnJsThreadFn>,
    dispatch: Option<DispatchFn>,
    is_js_thread: Option<IsJsThreadFn>,
    user_data: *const c_void,
) -> *const c_void {
    let Some(m) = m.as_ref() else {
        return std::ptr::null();
    };
    let (Some(on_js_thread), Some(dispatch), Some(is_js_thread)) =
        (on_js_thread, dispatch, is_js_thread)
    else {
        return std::ptr::null();
    };
    let Some(name) = cstr(callback_name) else {
        return std::ptr::null();
    };
    match m
        .module
        .make_callback_trampoline(&name, on_js_thread, dispatch, is_js_thread, user_data)
    {
        Ok(fn_ptr) => fn_ptr,
        Err(_) => std::ptr::null(),
    }
}

/// Build a vtable byte blob from ordered `(callback_name, fn_ptr)` pairs.
///
/// `callback_names` and `fn_ptrs` are parallel arrays of length `n`. Each name is
/// paired with the fn pointer at the same index to form a `VTableField`. Returns
/// the (leaked, program-lifetime) vtable pointer on success, or null on error
/// (null module, a null/not-UTF-8 name, or `build_vtable` failure).
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]. `callback_names` and
/// `fn_ptrs` must each be valid for `n` elements (or null when `n == 0`); each
/// name pointer must be a NUL-terminated C string. Each `fn_ptrs[i]` must be a
/// valid, non-null C function pointer; null fn pointers are NOT rejected and
/// storing them is undefined behavior. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_build_vtable(
    m: *mut UbrnJsiModule,
    struct_name: *const c_char,
    callback_names: *const *const c_char,
    fn_ptrs: *const *const c_void,
    n: usize,
) -> *const c_void {
    let Some(m) = m.as_ref() else {
        return std::ptr::null();
    };
    let Some(struct_name) = cstr(struct_name) else {
        return std::ptr::null();
    };
    let names = slice_or_empty(callback_names, n);
    let ptrs = slice_or_empty(fn_ptrs, n);
    let mut fields = Vec::with_capacity(n);
    for (&name, &fn_ptr) in names.iter().zip(ptrs.iter()) {
        let Some(callback_name) = cstr(name) else {
            return std::ptr::null();
        };
        fields.push(VTableField {
            callback_name,
            fn_ptr,
        });
    }
    match m.module.build_vtable(&struct_name, &fields) {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null(),
    }
}

/// Invoke a raw C function pointer using the named callback's signature.
///
/// `arg_blob` is the concatenation of each argument's `repr(C)` bytes; `sizes`
/// (length `n_args`) gives the byte length of each argument in order, splitting
/// `arg_blob` into one `Vec<u8>` per argument.
///
/// Returns 0 on success; non-zero on error:
/// - 1: module handle is null
/// - 2: `callback_name` is null or not UTF-8
/// - 3: the underlying call failed (unknown callback / unsupported signature)
/// - 4: argument sizes overflow / invalid
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]. `callback_name` must be
/// a NUL-terminated C string. `sizes` must be valid for `n_args` elements (or null
/// when `n_args == 0`); `arg_blob` must be valid for `sum(sizes)` bytes. An
/// overflowing `sizes` array (where the sum exceeds `usize`) is rejected with
/// code 4. `fn_ptr` must be a valid, non-null C function pointer matching the
/// callback signature; null fn pointers are NOT rejected and calling them is
/// undefined behavior. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_call_callback_ptr(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    fn_ptr: *const c_void,
    arg_blob: *const u8,
    sizes: *const usize,
    n_args: usize,
) -> i32 {
    let Some(m) = m.as_ref() else { return 1 };
    let Some(name) = cstr(callback_name) else {
        return 2;
    };

    let sizes = slice_or_empty(sizes, n_args);
    let Some(total) = sizes.iter().try_fold(0usize, |a, &s| a.checked_add(s)) else {
        return 4;
    };
    let blob = slice_or_empty(arg_blob, total);

    let mut arg_buffers: Vec<Vec<u8>> = Vec::with_capacity(n_args);
    let mut offset = 0usize;
    for &sz in sizes {
        // `blob` is sized to `total == sum(sizes)`, so `offset + sz <= total`.
        arg_buffers.push(blob[offset..offset + sz].to_vec());
        offset += sz;
    }

    match m.module.call_callback_ptr(&name, fn_ptr, arg_buffers) {
        Ok(()) => 0,
        Err(_) => 3,
    }
}

/// Query the C struct layout (computed via libffi in core) for a registered struct.
///
/// Writes the total struct size to `*out_total_size`, and the per-field
/// `offset`/`size` into `out_offsets[i]`/`out_sizes[i]` for up to `cap` fields.
/// Returns the struct's real field count (so a caller can detect truncation when
/// the return exceeds `cap`), or `-1` on error (null module, null name, or unknown
/// struct). Mirrors [`uniffi_runtime_core::Module::struct_field_offsets`].
///
/// # Safety
/// `m` must be a valid module from [`ubrn_jsi_register`]; `struct_name` a
/// NUL-terminated C string. `out_total_size` must be null or writable; `out_offsets`
/// and `out_sizes` must each be null or valid for `cap` elements. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_struct_field_offsets(
    m: *mut UbrnJsiModule,
    struct_name: *const c_char,
    out_total_size: *mut usize,
    out_offsets: *mut usize,
    out_sizes: *mut usize,
    cap: usize,
) -> i32 {
    let Some(m) = m.as_ref() else { return -1 };
    let Some(name) = cstr(struct_name) else {
        return -1;
    };
    let layout = match m.module.struct_field_offsets(&name) {
        Ok(l) => l,
        Err(_) => return -1,
    };
    if !out_total_size.is_null() {
        *out_total_size = layout.total_size;
    }
    let n = layout.fields.len();
    let write = n.min(cap);
    for (i, f) in layout.fields.iter().take(write).enumerate() {
        if !out_offsets.is_null() {
            *out_offsets.add(i) = f.offset;
        }
        if !out_sizes.is_null() {
            *out_sizes.add(i) = f.size;
        }
    }
    i32::try_from(n).unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_runtime_core::ffi_c_types::{RustBufferC, RustCallStatusC};

    #[test]
    fn maps_known_tags() {
        assert!(matches!(ffi_type_from_tag(5), Some(FfiTypeDesc::UInt32)));
        assert!(matches!(ffi_type_from_tag(10), Some(FfiTypeDesc::Float64)));
        assert!(matches!(ffi_type_from_tag(0), Some(FfiTypeDesc::Void)));
        // Tag 16 (UBRN_TY_RUSTCALLSTATUS) maps to the inline RustCallStatus desc,
        // used by struct-field marshalling (e.g. ForeignFutureResult<T>).
        assert!(matches!(
            ffi_type_from_tag(16),
            Some(FfiTypeDesc::RustCallStatus)
        ));
        // ffi_type_from_tag_named defers unnamed tags to ffi_type_from_tag, so it
        // resolves tag 16 the same way (name is ignored for non-named tags).
        assert!(matches!(
            ffi_type_from_tag_named(16, None),
            Ok(FfiTypeDesc::RustCallStatus)
        ));
        assert!(ffi_type_from_tag(250).is_none());
    }

    /// Rust side of the ABI drift guard: pins the same facts that
    /// `cpp/jsi-player-shim/abi_assert.cpp` pins on the C++ side (and that
    /// `ubrn_jsi.h`'s `UbrnFfiType` enum declares). The two must agree exactly or
    /// a registration silently corrupts memory.
    #[test]
    fn core_struct_sizes_match_header() {
        // UbrnRustBuffer is `{ u64; u64; *mut u8 }` = 24 bytes (header pins
        // sizeof(UbrnRustBuffer) == 24 and the field offsets).
        assert_eq!(std::mem::size_of::<RustBufferC>(), 24);
        // RustCallStatusC is `{ i8 code; u64; u64; *mut u8 }` with natural
        // 8-byte alignment = 32 bytes on a 64-bit target (code @0 + 7 pad,
        // then 8 + 8 + 8). The C++ shim's local `RustCallStatus` mirror relies
        // on this.
        assert_eq!(std::mem::size_of::<RustCallStatusC>(), 32);
    }

    /// The tag NUMBERS in `ubrn_jsi.h`'s `UbrnFfiType` enum are hard-coded here
    /// in `ffi_type_from_tag` / `ffi_type_from_tag_named`. Pin the full named/
    /// compound set (the drift-prone ones, 12..=16) so a header renumber that
    /// isn't mirrored here fails this test.
    #[test]
    fn tag_numbers_match_header() {
        // Scalars + Handle (0..=11) and RustBuffer (12) / RustCallStatus (16).
        assert!(matches!(ffi_type_from_tag(11), Some(FfiTypeDesc::Handle)));
        assert!(matches!(
            ffi_type_from_tag(12),
            Some(FfiTypeDesc::RustBuffer)
        ));
        assert!(matches!(
            ffi_type_from_tag(16),
            Some(FfiTypeDesc::RustCallStatus)
        ));
        // Named/compound tags carry a name and only resolve via *_named.
        assert!(matches!(
            ffi_type_from_tag_named(13, Some("Cb")),
            Ok(FfiTypeDesc::Callback(n)) if n == "Cb"
        ));
        assert!(matches!(
            ffi_type_from_tag_named(14, Some("S")),
            Ok(FfiTypeDesc::Struct(n)) if n == "S"
        ));
        // Tag 15 (Reference) always wraps a named Struct.
        assert!(matches!(
            ffi_type_from_tag_named(15, Some("V")),
            Ok(FfiTypeDesc::Reference(inner)) if matches!(*inner, FfiTypeDesc::Struct(ref n) if n == "V")
        ));
    }
}
