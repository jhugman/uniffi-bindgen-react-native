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
    desc_from_name, slot_size_align_for_name, CallbackDef, DispatchFn, FfiTypeDesc, FunctionDef,
    IsJsThreadFn, Module, ModuleSpec, OnJsThreadFn, RustBufferSymbols, StructDef, StructField,
    VTableField,
};

/// Build a slice from a caller-supplied array pointer + length.
///
/// `std::slice::from_raw_parts` requires a non-null, aligned pointer even when
/// `len == 0`, but a C++ caller may legitimately pass NULL for an empty array.
/// Yield an empty slice in that case without dereferencing the pointer.
///
/// # Safety
/// `ptr` must be null when `len == 0`, or else valid for `len` initialised
/// elements that stay live and unmutated for `'a`. Callers pick `'a`, so it must
/// not outlive the array the shim passed in — every caller here bounds it by the
/// export's own call.
unsafe fn slice_or_empty<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        &[]
    } else {
        // SAFETY: per this function's contract, `ptr` is valid for `len`
        // elements whenever `len != 0` (the `len == 0` case is handled above).
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Parse a parallel `(arg_tag_names, arg_type_names)` pair of length `n` into a
/// vector of `FfiTypeDesc`.
///
/// `arg_type_names` is parallel to `arg_tag_names`: it supplies the name for
/// `Callback`, `Struct` and `Reference` args and is ignored for scalars. A null
/// `arg_type_names` array is treated as "all names absent" (every name `None`).
///
/// # Safety
/// `arg_tag_names` must be valid for `n` elements (or null when `n == 0`), and
/// each element must be null or point to a NUL-terminated C string valid for
/// the duration of the call; a null element is reported as an error.
/// `arg_type_names`, when non-null, must satisfy the same contract (a null
/// element there just means the scalar arg has no name).
unsafe fn parse_args(
    arg_tag_names: *const *const c_char,
    arg_type_names: *const *const c_char,
    n: usize,
) -> Result<Vec<FfiTypeDesc>, String> {
    // SAFETY: per this function's contract, `arg_tag_names` is valid for `n`
    // elements.
    let tag_names = unsafe { slice_or_empty(arg_tag_names, n) };
    // Treat a null names array as all-None; otherwise borrow `n` name pointers.
    let names: &[*const c_char] = if arg_type_names.is_null() {
        &[]
    } else {
        // SAFETY: per this function's contract, `arg_type_names` (checked
        // non-null above) is valid for `n` elements.
        unsafe { slice_or_empty(arg_type_names, n) }
    };
    let mut args = Vec::with_capacity(n);
    for (i, &t) in tag_names.iter().enumerate() {
        // SAFETY: per this function's contract, each element of
        // `arg_tag_names` is null or points to a NUL-terminated C string,
        // satisfying cstr's contract.
        let tag = unsafe { cstr(t) }.ok_or_else(|| format!("null tag name at arg {i}"))?;
        // SAFETY: as above, for the parallel names array (a null element is
        // handled by cstr returning None).
        let name = names.get(i).copied().and_then(|p| unsafe { cstr(p) });
        args.push(desc_from_name(&tag, name.as_deref()).map_err(|e| e.to_string())?);
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
    /// Player tag names (length `n_args`), e.g. `"UInt8"` / `"Callback"`.
    pub arg_tag_names: *const *const c_char,
    pub n_args: usize,
    /// Parallel to `arg_tag_names` (length `n_args`): type name for Callback,
    /// Struct and Reference tags, null for scalars. The whole array may be
    /// null.
    pub arg_type_names: *const *const c_char,
    pub ret_tag_name: *const c_char,
    pub has_rust_call_status: u8,
}

#[repr(C)]
pub struct UbrnCallbackSpec {
    pub name: *const c_char,
    /// Player tag names (length `n_args`).
    pub arg_tag_names: *const *const c_char,
    pub arg_type_names: *const *const c_char,
    pub n_args: usize,
    pub ret_tag_name: *const c_char,
    pub has_rust_call_status: u8,
    pub out_return: u8,
    /// Type name for a `"Struct"` return, else null. Carries the struct name so
    /// a Struct-returning callback parses to `Struct(name)` instead of erroring;
    /// core ignores it when `out_return` is set. Mirrors `ubrn_jsi.h`'s field
    /// order (appended after `out_return`).
    pub ret_type_name: *const c_char,
}

#[repr(C)]
pub struct UbrnStructField {
    pub field_name: *const c_char,
    /// Player tag name, typically `"Callback"`.
    pub type_tag_name: *const c_char,
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

/// Borrow a NUL-terminated C string as a `&str` for the duration of the call.
///
/// The per-call exports read names through this rather than [`cstr`], so a name
/// nothing retains never costs an allocation.
///
/// # Safety
/// `s` must be null, or a NUL-terminated C string that stays valid and
/// unmutated for `'a`.
unsafe fn borrowed_cstr<'a>(s: *const c_char) -> Option<&'a str> {
    if s.is_null() {
        return None;
    }
    // SAFETY: non-null checked above; per this function's contract `s` is then
    // a NUL-terminated C string live for `'a`, which is `CStr::from_ptr`'s
    // requirement.
    unsafe { CStr::from_ptr(s) }.to_str().ok()
}

/// Read a NUL-terminated C string into an owned `String`.
///
/// # Safety
/// `s` must be a NUL-terminated C string from the shim, or null.
unsafe fn cstr(s: *const c_char) -> Option<String> {
    // SAFETY: per this function's contract `s` is null or a NUL-terminated C
    // string from the shim, which stays valid for this call — the whole extent
    // of the borrow, since it is copied before returning.
    unsafe { borrowed_cstr(s) }.map(str::to_owned)
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
    // SAFETY: per this function's contract `buf` points to at least `len`
    // writable bytes (non-null and `len != 0` checked above); `n <= len - 1`
    // so both the copy and the NUL write land within that region. `bytes`
    // is a distinct Rust-owned allocation, so it cannot overlap `buf`.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n);
        *buf.add(n) = 0;
    }
}

/// dlopen `lib_path`, resolve symbols, and build CIFs.
///
/// Returns a heap-allocated [`UbrnJsiModule`] on success, or null on error (with
/// a message written into `err_buf`).
///
/// # Safety
/// `lib_path` must be null or a NUL-terminated C string valid for the call.
/// `spec` must be null or a valid, aligned pointer to a live `UbrnModuleSpec`
/// for the call. The arrays referenced by `spec` (`functions`/`n_functions`,
/// `callbacks`/`n_callbacks`, `structs`/`n_structs`, and each entry's
/// `arg_tag_names`/`arg_type_names`/`n_args` or `fields`/`n_fields`) must each
/// have the declared lengths. Every `*const c_char` reachable from `spec`
/// (`rustbuffer_alloc`/`rustbuffer_free`/`rustbuffer_from_bytes`, each
/// function's/callback's `name` and `ret_tag_name`, each `arg_tag_names` and
/// `arg_type_names` element, each callback's `ret_type_name`, each struct's
/// `name`, and each field's `field_name`/`type_tag_name`/`type_name`) must be
/// null or point to a NUL-terminated C string valid for the call. `err_buf`
/// must be null or point to at least `err_len` writable bytes. Called by the
/// C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_register(
    lib_path: *const c_char,
    spec: *const UbrnModuleSpec,
    err_buf: *mut c_char,
    err_len: usize,
) -> *mut UbrnJsiModule {
    let result = (|| -> Result<*mut UbrnJsiModule, String> {
        // SAFETY: per this function's contract, `lib_path` is null or a
        // NUL-terminated C string, satisfying cstr's contract.
        let lib_path = unsafe { cstr(lib_path) }.ok_or("lib_path is null or not UTF-8")?;
        // SAFETY: per this function's contract, `spec` is a valid pointer.
        let spec = unsafe { spec.as_ref() }.ok_or("spec is null")?;

        // SAFETY: per this function's contract, `spec`'s `rustbuffer_*`
        // fields are each null or a NUL-terminated C string, satisfying
        // cstr's contract.
        let rustbuffer_symbols = unsafe {
            RustBufferSymbols {
                alloc: cstr(spec.rustbuffer_alloc).ok_or("rustbuffer_alloc missing")?,
                free: cstr(spec.rustbuffer_free).ok_or("rustbuffer_free missing")?,
                from_bytes: cstr(spec.rustbuffer_from_bytes)
                    .ok_or("rustbuffer_from_bytes missing")?,
            }
        };

        let mut functions = HashMap::new();
        // SAFETY: per this function's contract, the arrays referenced by
        // `spec` (here `functions`/`n_functions`) have the declared lengths.
        let fn_specs = unsafe { slice_or_empty(spec.functions, spec.n_functions) };
        for f in fn_specs {
            // SAFETY: `f` comes from `fn_specs`, itself validated above; per
            // this function's contract, `f.name` is null or a NUL-terminated
            // C string, satisfying cstr's contract.
            let name = unsafe { cstr(f.name) }.ok_or("function name missing")?;
            // SAFETY: per this function's contract, `f.arg_tag_names`/`n_args`
            // and `f.arg_type_names`/`n_args` have the declared lengths, and
            // each reachable element is null or a NUL-terminated C string —
            // satisfying parse_args's contract.
            let args = unsafe { parse_args(f.arg_tag_names, f.arg_type_names, f.n_args) }?;
            // SAFETY: per this function's contract, `f.ret_tag_name` is null or
            // a NUL-terminated C string, satisfying cstr's contract.
            let ret_tag = unsafe { cstr(f.ret_tag_name) }
                .ok_or_else(|| format!("function {name} return tag name missing"))?;
            let ret = desc_from_name(&ret_tag, None)
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
        // SAFETY: per this function's contract, the arrays referenced by
        // `spec` (here `callbacks`/`n_callbacks`) have the declared lengths.
        let cb_specs = unsafe { slice_or_empty(spec.callbacks, spec.n_callbacks) };
        for c in cb_specs {
            // SAFETY: `c` comes from `cb_specs`, itself validated above; per
            // this function's contract, `c.name` is null or a NUL-terminated
            // C string, satisfying cstr's contract.
            let name = unsafe { cstr(c.name) }.ok_or("callback name missing")?;
            // SAFETY: per this function's contract, `c.arg_tag_names`/`n_args`
            // and `c.arg_type_names`/`n_args` have the declared lengths, and
            // each reachable element is null or a NUL-terminated C string —
            // satisfying parse_args's contract.
            let args = unsafe { parse_args(c.arg_tag_names, c.arg_type_names, c.n_args) }?;
            // A callback's return may be a named Struct (written through the
            // out_return pointer); pass ret_type_name so it parses to Struct(name).
            // SAFETY: per this function's contract, `c.ret_type_name` is
            // null or a NUL-terminated C string, satisfying cstr's contract.
            let ret_name = unsafe { cstr(c.ret_type_name) };
            // SAFETY: per this function's contract, `c.ret_tag_name` is null or
            // a NUL-terminated C string, satisfying cstr's contract.
            let ret_tag = unsafe { cstr(c.ret_tag_name) }
                .ok_or_else(|| format!("callback {name} return tag name missing"))?;
            let ret = desc_from_name(&ret_tag, ret_name.as_deref())
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
        // SAFETY: per this function's contract, the arrays referenced by
        // `spec` (here `structs`/`n_structs`) have the declared lengths.
        let struct_specs = unsafe { slice_or_empty(spec.structs, spec.n_structs) };
        for s in struct_specs {
            // SAFETY: `s` comes from `struct_specs`, itself validated above;
            // per this function's contract, `s.name` is null or a
            // NUL-terminated C string, satisfying cstr's contract.
            let name = unsafe { cstr(s.name) }.ok_or("struct name missing")?;
            // SAFETY: `s.fields`/`s.n_fields` are a caller-supplied array
            // with the declared length, per this function's contract.
            let field_specs = unsafe { slice_or_empty(s.fields, s.n_fields) };
            let mut fields = Vec::with_capacity(s.n_fields);
            for field in field_specs {
                // SAFETY: `field` comes from `field_specs`, itself validated
                // above; per this function's contract, `field.field_name` is
                // null or a NUL-terminated C string, satisfying cstr's
                // contract.
                let field_name =
                    unsafe { cstr(field.field_name) }.ok_or("struct field name missing")?;
                // SAFETY: per this function's contract, `field.type_name` is
                // null or a NUL-terminated C string, satisfying cstr's
                // contract.
                let type_name = unsafe { cstr(field.type_name) };
                // SAFETY: per this function's contract, `field.type_tag_name`
                // is null or a NUL-terminated C string, satisfying cstr's
                // contract.
                let type_tag = unsafe { cstr(field.type_tag_name) }.ok_or_else(|| {
                    format!("struct {name} field {field_name}: type tag name missing")
                })?;
                let field_type = desc_from_name(&type_tag, type_name.as_deref())
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
            // SAFETY: per this function's contract, `err_buf` is null or
            // points to at least `err_len` writable bytes, satisfying
            // write_err's contract.
            unsafe { write_err(err_buf, err_len, &msg) };
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
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`].
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_alloc(
    m: *mut UbrnJsiModule,
    size: u64,
) -> uniffi_runtime_core::ffi_c_types::RustBufferC {
    use uniffi_runtime_core::ffi_c_types::RustBufferC;
    // SAFETY: per this function's contract, `m` must be a valid module from
    // `ubrn_jsi_register`, or null (handled by `as_ref` returning `None`).
    let Some(m) = (unsafe { m.as_ref() }) else {
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
    match m.module.rustbuffer_alloc(size as i32) {
        Ok(rb) => rb,
        Err(_) => RustBufferC {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        },
    }
}

/// Copy `len` borrowed bytes into a new Rust-owned buffer via `rustbuffer_from_bytes`.
///
/// Returns a [`RustBufferC`] whose `len == len` on success, or a zeroed buffer on error.
/// The caller is responsible for eventually freeing the buffer with
/// [`ubrn_jsi_rustbuffer_free`].
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `data` must be valid for `len` bytes (or null when `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_from_bytes(
    m: *mut UbrnJsiModule,
    data: *const u8,
    len: usize,
) -> uniffi_runtime_core::ffi_c_types::RustBufferC {
    use uniffi_runtime_core::ffi_c_types::RustBufferC;
    // SAFETY: per this function's contract, `m` must be a valid module from
    // `ubrn_jsi_register`, or null (handled by `as_ref` returning `None`).
    let Some(m) = (unsafe { m.as_ref() }) else {
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
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `buf` must have been allocated by the same module (via
/// [`ubrn_jsi_rustbuffer_alloc`] or [`ubrn_jsi_rustbuffer_from_bytes`]).
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_rustbuffer_free(
    m: *mut UbrnJsiModule,
    buf: uniffi_runtime_core::ffi_c_types::RustBufferC,
) {
    // SAFETY: per this function's contract, `m` must be a valid module from
    // `ubrn_jsi_register`, or null (handled by `as_ref` returning `None`).
    if let Some(m) = unsafe { m.as_ref() } {
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
        // SAFETY: per this function's contract (checked non-null above), `m`
        // is a pointer returned by `ubrn_jsi_register`, called at most once —
        // exactly what `Box::from_raw` requires to reclaim it.
        drop(unsafe { Box::from_raw(m) });
    }
}

/// Stop a module serving its frontend: set the unloading flag and invoke the
/// registered abort hook.
///
/// Idempotent, and a null handle is a no-op. Nothing is drained, closed or
/// freed, so the handle stays valid; every trampoline already handed out now
/// returns without calling into the frontend, zeroing whatever return bytes it
/// has. An `out_return` callback has none, so Rust reads back
/// `FfiDefault::ffi_default()` with `call_status.code` still 0 — a successful
/// empty return, not a reported failure. No new trampoline can be built.
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_disarm(m: *mut UbrnJsiModule) {
    // SAFETY: per this function's contract, `m` must be a valid module from
    // `ubrn_jsi_register`, or null (handled by `as_ref` returning `None`).
    if let Some(m) = unsafe { m.as_ref() } {
        let _ = m.module.disarm();
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
/// - 5: the underlying FFI call failed (e.g. module unloading), or the
///   return buffer was too small (unreachable: the shim sizes it from core's
///   own geometry export)
/// - 6: the return type has no byte representation core can write
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `fn_name` must be null or a NUL-terminated C string.
/// `args` and `arg_sizes` must each be valid for `n_args` elements (or null when
/// `n_args == 0`); each `args[i]` (for `i < n_args`) must be non-null and valid
/// for `arg_sizes[i]` bytes. `status` is either null or a pointer to a
/// caller-allocated `RustCallStatus` that stays alive for the duration of the
/// call. `out_ret` must be null or point to at least `out_ret_size` writable
/// bytes. Called by the C++ shim.
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
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return 1;
    };
    // Borrowed rather than read through `cstr`: this runs on every FFI call,
    // and `prepare_call` reads the name without retaining it, so an owned copy
    // would be an allocation per call.
    //
    // SAFETY: per this function's contract, `fn_name` is null or a
    // NUL-terminated C string, satisfying borrowed_cstr's contract.
    let Some(name) = (unsafe { borrowed_cstr(fn_name) }) else {
        return 2;
    };

    let mut call = match m.module.prepare_call(name) {
        Ok(c) => c,
        Err(_) => return 3,
    };

    // SAFETY: per this function's contract, `args` is valid for `n_args`
    // elements (or null when `n_args == 0`), satisfying slice_or_empty's
    // contract.
    let arg_ptrs = unsafe { slice_or_empty(args, n_args) };
    // SAFETY: per this function's contract, `arg_sizes` is valid for
    // `n_args` elements (or null when `n_args == 0`), satisfying
    // slice_or_empty's contract.
    let arg_lens = unsafe { slice_or_empty(arg_sizes, n_args) };
    for i in 0..n_args {
        let Ok(dst) = call.arg_slot(i) else { return 4 };
        let copy_len = arg_lens[i].min(dst.len());
        // SAFETY: per this function's contract, `args[i]` is non-null and
        // valid for `arg_lens[i]` bytes; `copy_len <= arg_lens[i]`.
        let src = unsafe { std::slice::from_raw_parts(arg_ptrs[i], copy_len) };
        dst[..src.len()].copy_from_slice(src);
    }

    if !status.is_null() {
        if let Some(rcs_slot) = call.rust_call_status_slot() {
            slot::write_pointer(rcs_slot, status as *const c_void);
        }
    }

    let out: &mut [u8] = if out_ret.is_null() || out_ret_size == 0 {
        &mut []
    } else {
        // SAFETY: per this function's contract, `out_ret` (checked non-null
        // and `out_ret_size != 0` above) is valid for `out_ret_size` writable
        // bytes.
        unsafe { std::slice::from_raw_parts_mut(out_ret, out_ret_size) }
    };
    match m.module.call(call, out) {
        Ok(_) => 0,
        // UnsupportedType is the only reachable "couldn't write the return"
        // case (an undersized `out` can't happen: the shim sizes it from
        // core's own geometry export); every other error is a call failure.
        Err(uniffi_runtime_core::Error::UnsupportedType(_)) => 6,
        Err(_) => 5,
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
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `callback_name` must be a NUL-terminated C string.
/// The seam fn pointers, when non-null, must point to functions matching the
/// corresponding C typedef. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_make_trampoline(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    on_js_thread: Option<OnJsThreadFn>,
    dispatch: Option<DispatchFn>,
    is_js_thread: Option<IsJsThreadFn>,
    user_data: *const c_void,
) -> *const c_void {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return std::ptr::null();
    };
    let (Some(on_js_thread), Some(dispatch), Some(is_js_thread)) =
        (on_js_thread, dispatch, is_js_thread)
    else {
        return std::ptr::null();
    };
    // SAFETY: per this function's contract, `callback_name` is a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(name) = (unsafe { cstr(callback_name) }) else {
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

/// The trampoline already built for `(callback_name, identity)`, or null if none.
///
/// `identity` names one JS function. The shim mints it — core cannot observe JS
/// identity — and keeps it unique across every module it registers, since one
/// JS function marshalled into two modules carries the one number into both.
/// Core only keys the reuse map on the pair. Null is also the answer on error
/// (null module, `callback_name` null/not-UTF-8); a miss and an error both mean
/// "build one".
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `callback_name` must be a NUL-terminated C string.
/// Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_trampoline_for(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    identity: u64,
) -> *const c_void {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return std::ptr::null();
    };
    // Borrowed rather than read through `cstr`: this runs on every callback
    // marshal, and `trampoline_for` reads the name without retaining it, so an
    // owned copy would be an allocation per lookup.
    //
    // SAFETY: per this function's contract, `callback_name` is null or a
    // NUL-terminated C string, satisfying borrowed_cstr's contract.
    let Some(name) = (unsafe { borrowed_cstr(callback_name) }) else {
        return std::ptr::null();
    };
    m.module
        .trampoline_for(name, identity)
        .unwrap_or(std::ptr::null())
}

/// Record `fn_ptr` as the trampoline for `(callback_name, identity)`.
///
/// A later [`ubrn_jsi_trampoline_for`] with the same pair returns it instead of
/// building — and leaking — another. Silently does nothing on error (null
/// module, `callback_name` null/not-UTF-8): the caller loses reuse, nothing
/// more. `fn_ptr` is stored as given, null included.
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `callback_name` must be a NUL-terminated C string.
/// Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_remember_trampoline(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    identity: u64,
    fn_ptr: *const c_void,
) {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return;
    };
    // SAFETY: per this function's contract, `callback_name` is a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(name) = (unsafe { cstr(callback_name) }) else {
        return;
    };
    m.module.remember_trampoline(&name, identity, fn_ptr);
}

/// Build a vtable byte blob from ordered `(callback_name, fn_ptr)` pairs.
///
/// `callback_names` and `fn_ptrs` are parallel arrays of length `n`. Each name is
/// paired with the fn pointer at the same index to form a `VTableField`. Returns
/// the (leaked, program-lifetime) vtable pointer on success, or null on error
/// (null module, a null/not-UTF-8 name, or `build_vtable` failure).
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `callback_names` and `fn_ptrs` must each be valid for
/// `n` elements (or null when `n == 0`); each name pointer must be a
/// NUL-terminated C string. Each `fn_ptrs[i]` must be a valid, non-null C
/// function pointer; null fn pointers are NOT rejected and storing them is
/// undefined behavior. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_build_vtable(
    m: *mut UbrnJsiModule,
    struct_name: *const c_char,
    callback_names: *const *const c_char,
    fn_ptrs: *const *const c_void,
    n: usize,
) -> *const c_void {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return std::ptr::null();
    };
    // SAFETY: per this function's contract, `struct_name` is a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(struct_name) = (unsafe { cstr(struct_name) }) else {
        return std::ptr::null();
    };
    // SAFETY: per this function's contract, `callback_names` is valid for
    // `n` elements (or null when `n == 0`), satisfying slice_or_empty's
    // contract.
    let names = unsafe { slice_or_empty(callback_names, n) };
    // SAFETY: per this function's contract, `fn_ptrs` is valid for `n`
    // elements (or null when `n == 0`), satisfying slice_or_empty's
    // contract.
    let ptrs = unsafe { slice_or_empty(fn_ptrs, n) };
    let mut fields = Vec::with_capacity(n);
    for (&name, &fn_ptr) in names.iter().zip(ptrs.iter()) {
        // SAFETY: per this function's contract, each element of
        // `callback_names` is a NUL-terminated C string, satisfying cstr's
        // contract.
        let Some(callback_name) = (unsafe { cstr(name) }) else {
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
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]. `callback_name` must be a NUL-terminated C string.
/// `sizes` must be valid for `n_args` elements (or null when `n_args == 0`);
/// `arg_blob` must be valid for `sum(sizes)` bytes. An overflowing `sizes` array
/// (where the sum exceeds `usize`) is rejected with code 4. `fn_ptr` must be a
/// valid, non-null C function pointer matching the callback signature; null fn
/// pointers are NOT rejected and calling them is undefined behavior. Called by
/// the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_call_callback_ptr(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    fn_ptr: *const c_void,
    arg_blob: *const u8,
    sizes: *const usize,
    n_args: usize,
) -> i32 {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return 1;
    };
    // Borrowed rather than read through `cstr`: this runs on every completer
    // invocation, and `call_callback_ptr` reads the name without retaining it.
    //
    // SAFETY: per this function's contract, `callback_name` is a
    // NUL-terminated C string, satisfying borrowed_cstr's contract.
    let Some(name) = (unsafe { borrowed_cstr(callback_name) }) else {
        return 2;
    };

    // SAFETY: per this function's contract, `sizes` is valid for `n_args`
    // elements (or null when `n_args == 0`), satisfying slice_or_empty's
    // contract.
    let sizes = unsafe { slice_or_empty(sizes, n_args) };
    let Some(total) = sizes.iter().try_fold(0usize, |a, &s| a.checked_add(s)) else {
        return 4;
    };
    // SAFETY: per this function's contract, `arg_blob` is valid for
    // `sum(sizes)` bytes, i.e. `total`, satisfying slice_or_empty's contract.
    let blob = unsafe { slice_or_empty(arg_blob, total) };

    let mut arg_buffers: Vec<Vec<u8>> = Vec::with_capacity(n_args);
    let mut offset = 0usize;
    for &sz in sizes {
        // `blob` is sized to `total == sum(sizes)`, so `offset + sz <= total`.
        arg_buffers.push(blob[offset..offset + sz].to_vec());
        offset += sz;
    }

    match m.module.call_callback_ptr(name, fn_ptr, arg_buffers) {
        Ok(()) => 0,
        Err(_) => 3,
    }
}

/// Publish one `(total_size, [offset/size]*)` table through the out-params the
/// two layout exports share, and return the real slot count.
///
/// `slots` must yield exactly `n` items; only the first `cap` are written, so a
/// return greater than `cap` tells the caller its arrays were truncated. `-1`
/// stands for "count does not fit an `i32`", joining the exports' error return.
///
/// # Safety
/// `out_total_size` must be null or writable; `out_offsets` and `out_sizes`
/// must each be null or valid for `cap` elements.
unsafe fn publish_slot_table(
    total_size: usize,
    slots: impl Iterator<Item = (usize, usize)>,
    n: usize,
    out_total_size: *mut usize,
    out_offsets: *mut usize,
    out_sizes: *mut usize,
    cap: usize,
) -> i32 {
    if !out_total_size.is_null() {
        // SAFETY: per this function's contract, `out_total_size` (checked
        // non-null above) is writable.
        unsafe { *out_total_size = total_size };
    }
    for (i, (offset, size)) in slots.take(cap).enumerate() {
        if !out_offsets.is_null() {
            // SAFETY: per this function's contract, `out_offsets` (checked
            // non-null above) is valid for `cap` elements, and `take(cap)`
            // bounds this loop to `cap` iterations, so `i < cap`.
            unsafe { *out_offsets.add(i) = offset };
        }
        if !out_sizes.is_null() {
            // SAFETY: as for `out_offsets` above.
            unsafe { *out_sizes.add(i) = size };
        }
    }
    i32::try_from(n).unwrap_or(-1)
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
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]; `struct_name` a NUL-terminated C string.
/// `out_total_size` must be null or writable; `out_offsets` and `out_sizes`
/// must each be null or valid for `cap` elements. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_struct_field_offsets(
    m: *mut UbrnJsiModule,
    struct_name: *const c_char,
    out_total_size: *mut usize,
    out_offsets: *mut usize,
    out_sizes: *mut usize,
    cap: usize,
) -> i32 {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return -1;
    };
    // SAFETY: per this function's contract, `struct_name` is a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(name) = (unsafe { cstr(struct_name) }) else {
        return -1;
    };
    let layout = match m.module.struct_field_offsets(&name) {
        Ok(l) => l,
        Err(_) => return -1,
    };
    // SAFETY: this function's out-param contract is publish_slot_table's.
    unsafe {
        publish_slot_table(
            layout.total_size,
            layout.fields.iter().map(|f| (f.offset, f.size)),
            layout.fields.len(),
            out_total_size,
            out_offsets,
            out_sizes,
            cap,
        )
    }
}

/// Size and alignment of one argument slot for a player tag name.
///
/// Writes them to `*out_size`/`*out_align` and returns `true`. Returns `false`,
/// leaving both untouched, for a null/non-UTF-8 name or a name with no slot
/// representation (an unrecognized tag, or a bare `Struct`, which only
/// travels behind a pointer); a caller then treats the slot as size 0,
/// align 1. Mirrors [`uniffi_runtime_core::slot_size_align_for_name`], the
/// single geometry table.
///
/// # Safety
/// `tag_name` must be null or a NUL-terminated C string. `out_size` and
/// `out_align` must each be null or point to a writable `usize`. Called by
/// the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_scalar_slot_size_align(
    tag_name: *const c_char,
    out_size: *mut usize,
    out_align: *mut usize,
) -> bool {
    // SAFETY: per this function's contract, `tag_name` is null or a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(name) = (unsafe { cstr(tag_name) }) else {
        return false;
    };
    let Some((size, align)) = slot_size_align_for_name(&name) else {
        return false;
    };
    if !out_size.is_null() {
        // SAFETY: per this function's contract, `out_size` (checked non-null
        // above) points to a writable `usize`.
        unsafe { *out_size = size };
    }
    if !out_align.is_null() {
        // SAFETY: per this function's contract, `out_align` (checked non-null
        // above) points to a writable `usize`.
        unsafe { *out_align = align };
    }
    true
}

/// Query the argument-buffer layout core's trampoline packs for a callback.
///
/// Writes the whole buffer's byte length to `*out_total_size`, and the per-slot
/// `offset`/`size` into `out_offsets[i]`/`out_sizes[i]` for up to `cap` slots,
/// in CIF order: declared args, then the out-return pointer (if any), then the
/// `RustCallStatus` pointer (if any). Returns the callback's real slot count (so
/// a caller can detect truncation when the return exceeds `cap`), or `-1` on
/// error (null module, null name, or unknown callback). Mirrors
/// [`uniffi_runtime_core::Module::callback_arg_layout`].
///
/// # Safety
/// `m` must be null or a valid pointer to a live `UbrnJsiModule` from
/// [`ubrn_jsi_register`]; `callback_name` a NUL-terminated C string.
/// `out_total_size` must be null or writable; `out_offsets` and `out_sizes`
/// must each be null or valid for `cap` elements. Called by the C++ shim.
#[no_mangle]
pub unsafe extern "C" fn ubrn_jsi_callback_arg_layout(
    m: *mut UbrnJsiModule,
    callback_name: *const c_char,
    out_total_size: *mut usize,
    out_offsets: *mut usize,
    out_sizes: *mut usize,
    cap: usize,
) -> i32 {
    // SAFETY: per this function's contract, `m` is null or a valid pointer
    // to a live `UbrnJsiModule`.
    let Some(m) = (unsafe { m.as_ref() }) else {
        return -1;
    };
    // SAFETY: per this function's contract, `callback_name` is a
    // NUL-terminated C string, satisfying cstr's contract.
    let Some(name) = (unsafe { cstr(callback_name) }) else {
        return -1;
    };
    let layout = match m.module.callback_arg_layout(&name) {
        Ok(l) => l,
        Err(_) => return -1,
    };
    // The out-return pointer is already the last of `arg_slots`; the
    // RustCallStatus pointer follows it, matching the CIF's argument order.
    let slots = layout
        .arg_slots
        .iter()
        .chain(layout.rust_call_status_slot.iter());
    let n = layout.arg_slots.len() + usize::from(layout.rust_call_status_slot.is_some());
    // SAFETY: this function's out-param contract is publish_slot_table's.
    unsafe {
        publish_slot_table(
            layout.total_size,
            slots.map(|s| (s.offset, s.size)),
            n,
            out_total_size,
            out_offsets,
            out_sizes,
            cap,
        )
    }
}

// --- ABI drift guard ---------------------------------------------------------
//
// Both blocks below sit at module scope, outside `cfg(test)`, so they
// const-evaluate on every build of this crate for every target —
// `cargo check --target <32-bit triple>` included, with nothing to run.

/// Rust side of the ABI drift guard: pins the same facts that
/// `cpp/jsi-player-shim/abi_assert.cpp` pins on the C++ side. The two must
/// agree exactly or a registration silently corrupts memory.
///
/// Spelled in pointer width and u64 alignment rather than 64-bit numbers,
/// because a stock React Native Android build also compiles armeabi-v7a and
/// x86.
const _: () = {
    use std::mem::{align_of, offset_of, size_of};
    use uniffi_runtime_core::ffi_c_types::{RustBufferC, RustCallStatusC};

    /// One pointer, the unit every spec-struct offset below is counted in.
    const PTR: usize = size_of::<*const c_void>();
    const U64: usize = size_of::<u64>();
    /// The alignment a `u64` *member* imposes on its struct: 8 on 64-bit and
    /// armeabi-v7a, 4 on x86.
    const U64_ALIGN: usize = align_of::<u64>();

    // RustBufferC is `{ u64 capacity; u64 len; *mut u8 data }`: the two
    // u64s put `data` at 16, and the tail pads out to the alignment a
    // u64 member imposes — 24 bytes on 64-bit and armeabi-v7a, 20 on
    // x86, where that alignment is 4.
    assert!(
        offset_of!(RustBufferC, capacity) == 0,
        "RustBufferC.capacity"
    );
    assert!(offset_of!(RustBufferC, len) == U64, "RustBufferC.len");
    assert!(offset_of!(RustBufferC, data) == 2 * U64, "RustBufferC.data");
    assert!(
        size_of::<RustBufferC>() == (2 * U64 + PTR).next_multiple_of(U64_ALIGN),
        "RustBufferC is two u64s plus a pointer, padded to u64 alignment"
    );
    // RustCallStatusC is `{ i8 code; u64; u64; *mut u8 }`: `code` pads
    // out to the u64 alignment, then the inlined RustBuffer fields
    // follow. The C++ shim's local `RustCallStatus` mirror relies on
    // this.
    assert!(
        size_of::<RustCallStatusC>() == (U64_ALIGN + 2 * U64 + PTR).next_multiple_of(U64_ALIGN),
        "RustCallStatusC is a padded i8 followed by a RustBuffer"
    );
};

/// The spec structs the shim builds in native memory and hands to
/// `ubrn_jsi_register`. `abi_assert.cpp` pins these offsets against
/// `ubrn_jsi.h`; this pins the same numbers against the Rust mirrors, so a
/// field reordered on one side alone fails a build rather than reading a
/// tag name out of a pointer field.
///
/// Every member is pointer-sized bar the trailing `u8` flags, so the
/// offsets are counts of pointers and hold on every width.
const _: () = {
    use std::mem::{offset_of, size_of};

    /// One pointer, the unit every spec-struct offset below is counted in.
    const PTR: usize = size_of::<*const c_void>();

    assert!(size_of::<usize>() == PTR, "usize must be pointer-sized");

    assert!(offset_of!(UbrnFunctionSpec, name) == 0, "FunctionSpec.name");
    assert!(
        offset_of!(UbrnFunctionSpec, arg_tag_names) == PTR,
        "FunctionSpec.arg_tag_names"
    );
    assert!(
        offset_of!(UbrnFunctionSpec, n_args) == 2 * PTR,
        "FunctionSpec.n_args"
    );
    assert!(
        offset_of!(UbrnFunctionSpec, arg_type_names) == 3 * PTR,
        "FunctionSpec.arg_type_names"
    );
    assert!(
        offset_of!(UbrnFunctionSpec, ret_tag_name) == 4 * PTR,
        "FunctionSpec.ret_tag_name"
    );
    assert!(
        offset_of!(UbrnFunctionSpec, has_rust_call_status) == 5 * PTR,
        "FunctionSpec.has_rust_call_status"
    );
    // The trailing u8 pads out to pointer alignment.
    assert!(
        size_of::<UbrnFunctionSpec>() == 6 * PTR,
        "FunctionSpec size"
    );

    assert!(offset_of!(UbrnCallbackSpec, name) == 0, "CallbackSpec.name");
    assert!(
        offset_of!(UbrnCallbackSpec, arg_tag_names) == PTR,
        "CallbackSpec.arg_tag_names"
    );
    assert!(
        offset_of!(UbrnCallbackSpec, arg_type_names) == 2 * PTR,
        "CallbackSpec.arg_type_names"
    );
    assert!(
        offset_of!(UbrnCallbackSpec, n_args) == 3 * PTR,
        "CallbackSpec.n_args"
    );
    assert!(
        offset_of!(UbrnCallbackSpec, ret_tag_name) == 4 * PTR,
        "CallbackSpec.ret_tag_name"
    );
    assert!(
        offset_of!(UbrnCallbackSpec, has_rust_call_status) == 5 * PTR,
        "CallbackSpec.has_rust_call_status"
    );
    // The two u8 flags share one pointer-sized slot.
    assert!(
        offset_of!(UbrnCallbackSpec, out_return) == 5 * PTR + 1,
        "CallbackSpec.out_return"
    );
    assert!(
        offset_of!(UbrnCallbackSpec, ret_type_name) == 6 * PTR,
        "CallbackSpec.ret_type_name"
    );
    assert!(
        size_of::<UbrnCallbackSpec>() == 7 * PTR,
        "CallbackSpec size"
    );

    assert!(
        offset_of!(UbrnStructField, field_name) == 0,
        "StructField.field_name"
    );
    assert!(
        offset_of!(UbrnStructField, type_tag_name) == PTR,
        "StructField.type_tag_name"
    );
    assert!(
        offset_of!(UbrnStructField, type_name) == 2 * PTR,
        "StructField.type_name"
    );
    assert!(size_of::<UbrnStructField>() == 3 * PTR, "StructField size");

    assert!(offset_of!(UbrnStructSpec, name) == 0, "StructSpec.name");
    assert!(
        offset_of!(UbrnStructSpec, fields) == PTR,
        "StructSpec.fields"
    );
    assert!(
        offset_of!(UbrnStructSpec, n_fields) == 2 * PTR,
        "StructSpec.n_fields"
    );
    assert!(size_of::<UbrnStructSpec>() == 3 * PTR, "StructSpec size");

    assert!(
        offset_of!(UbrnModuleSpec, rustbuffer_alloc) == 0,
        "ModuleSpec.rustbuffer_alloc"
    );
    assert!(
        offset_of!(UbrnModuleSpec, rustbuffer_free) == PTR,
        "ModuleSpec.rustbuffer_free"
    );
    assert!(
        offset_of!(UbrnModuleSpec, rustbuffer_from_bytes) == 2 * PTR,
        "ModuleSpec.rustbuffer_from_bytes"
    );
    assert!(
        offset_of!(UbrnModuleSpec, functions) == 3 * PTR,
        "ModuleSpec.functions"
    );
    assert!(
        offset_of!(UbrnModuleSpec, n_functions) == 4 * PTR,
        "ModuleSpec.n_functions"
    );
    assert!(
        offset_of!(UbrnModuleSpec, callbacks) == 5 * PTR,
        "ModuleSpec.callbacks"
    );
    assert!(
        offset_of!(UbrnModuleSpec, n_callbacks) == 6 * PTR,
        "ModuleSpec.n_callbacks"
    );
    assert!(
        offset_of!(UbrnModuleSpec, structs) == 7 * PTR,
        "ModuleSpec.structs"
    );
    assert!(
        offset_of!(UbrnModuleSpec, n_structs) == 8 * PTR,
        "ModuleSpec.n_structs"
    );
    assert!(size_of::<UbrnModuleSpec>() == 9 * PTR, "ModuleSpec size");
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// The C++ shim sizes every flat arg slot from this export, so it must
    /// answer with core's geometry and must report "no slot" (rather than a
    /// guess) for the names core refuses.
    #[test]
    fn scalar_slot_size_align_reports_core_geometry() {
        let mut size = usize::MAX;
        let mut align = usize::MAX;
        let name = CString::new("UInt16").unwrap();
        // SAFETY: `name` is a live NUL-terminated C string; both out params
        // point to live, writable locals.
        let ok = unsafe { ubrn_jsi_scalar_slot_size_align(name.as_ptr(), &mut size, &mut align) };
        assert!(ok);
        assert_eq!((size, align), (2, 2));

        let name = CString::new("RustBuffer").unwrap();
        // SAFETY: `name` is a live NUL-terminated C string; both out params
        // point to live, writable locals.
        let ok = unsafe { ubrn_jsi_scalar_slot_size_align(name.as_ptr(), &mut size, &mut align) };
        assert!(ok);
        assert_eq!((size, align), (24, 8));

        // A bare Struct has no flat-slot form, and "NotATag" is not a tag at
        // all; both leave the out params alone.
        for name in ["Struct", "NotATag"] {
            size = usize::MAX;
            align = usize::MAX;
            let c = CString::new(name).unwrap();
            // SAFETY: `c` is a live NUL-terminated C string; both out params
            // point to live, writable locals.
            let ok = unsafe { ubrn_jsi_scalar_slot_size_align(c.as_ptr(), &mut size, &mut align) };
            assert!(!ok, "{name} should have no slot geometry");
            assert_eq!((size, align), (usize::MAX, usize::MAX));
        }

        // Null out params are allowed.
        let name = CString::new("Handle").unwrap();
        // SAFETY: `name` is a live NUL-terminated C string; null out params
        // are explicitly permitted by the function's contract.
        assert!(unsafe {
            ubrn_jsi_scalar_slot_size_align(
                name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        });

        // A null tag_name is also permitted, and reports "no slot".
        // SAFETY: null is explicitly permitted by the function's contract.
        assert!(!unsafe {
            ubrn_jsi_scalar_slot_size_align(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        });
    }
}
