/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! # The Orchestrator
//!
//! This module is the entry point for the entire FFI bridge. The [`register`] function
//! accepts a JavaScript object that *describes* a native library — its exported functions,
//! callback signatures, and struct (VTable) layouts — and returns a JavaScript object whose
//! methods call directly into that library. Every such call passes through
//! [`call_ffi_function`], which marshals JS values to their C representations, invokes the
//! target symbol via libffi, and marshals the result back to JS.
//!
//! ## The Registration Pipeline
//!
//! [`register`] proceeds in three phases:
//!
//! 1. **Resolve RustBuffer management symbols** (`alloc`, `free`, `from_bytes`).
//!    These are needed by any function that passes or returns a `RustBuffer`.
//! 2. **Parse callback and struct definitions** from the JS description object.
//!    Each definition records the argument types, return type, and
//!    `hasRustCallStatus` flag so that call-time marshalling knows what to do.
//! 3. **For each exported function:** parse its signature, look up its symbol via
//!    `dlsym`, build a libffi CIF, and create a JS closure that captures
//!    everything needed to perform the call at runtime.
//!
//! ## The Call Pipeline (`call_ffi_function`)
//!
//! When JS calls one of the generated methods, `call_ffi_function` runs:
//!
//! 1. **Marshal JS arguments to Rust values.** Four cases:
//!    - *RustBuffer*: convert a `Uint8Array` to a `RustBufferC` via `rustbuffer_from_bytes`.
//!    - *Callback*: create a libffi closure backed by a trampoline (see `callback.rs`),
//!      yielding a C function pointer.
//!    - *Reference(Struct)*: build a VTable struct (see `structs.rs`) from a JS object.
//!    - *Scalars*: convert via [`marshal::js_to_boxed`].
//! 2. **Build the libffi argument vector** from the marshalled values.
//! 3. **Append a `RustCallStatus` pointer** as the final C argument, if the function
//!    declares `hasRustCallStatus`.
//! 4. **Call via libffi** (`cif.call`).
//! 5. **Write back `RustCallStatus`** to the JS status object, including any error
//!    buffer contents (converted to `Uint8Array`).
//! 6. **Marshal the return value** back to JS.
//!
//! ## On intentional leaks
//!
//! Callback closures and their userdata are intentionally leaked (`Box::into_raw` +
//! `mem::forget`). The native library may invoke the callback from an arbitrary thread
//! at an arbitrary time after `call_ffi_function` returns, so we cannot determine the
//! callback's lifetime statically. This is a deliberate trade-off: a small, bounded
//! amount of memory is leaked per callback registration in exchange for soundness.

use std::any::Any;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use libffi::middle::{arg, Arg, Cif, Closure, CodePtr};
use napi::bindgen_prelude::*;
use napi::{JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::callback::{self, raw_arg_to_js, CallbackDef, RawCallbackArg, TrampolineUserdata};
use crate::cif::ffi_type_for;
use crate::ffi_c_types::{RustBufferC, RustBufferOps, RustCallStatusC};
use crate::ffi_type::FfiTypeDesc;
use crate::library::LibraryHandle;
use crate::marshal;
use crate::napi_utils;
use crate::structs;

/// A fully-resolved FFI function: everything needed to invoke it at call time.
struct ResolvedFfiFunc {
    cif: Rc<Cif>,
    symbol_ptr: *const c_void,
    arg_types: Rc<Vec<FfiTypeDesc>>,
    ret_type: FfiTypeDesc,
    has_rust_call_status: bool,
}

/// Shared definition context: callback and struct definitions parsed at registration time.
struct DefinitionContext {
    callbacks: Rc<HashMap<String, CallbackDef>>,
    structs: Rc<HashMap<String, structs::StructDef>>,
}

/// Resolve the three RustBuffer management symbols (`alloc`, `free`, `from_bytes`)
/// from the `symbols` property of the JS definitions object.
///
/// Only `from_bytes` and `free` are returned; `alloc` is resolved purely to validate
/// that the symbol exists in the loaded library. We call `from_bytes` at marshalling
/// time (JS `Uint8Array` to `RustBufferC`) and `free` after copying data out.
fn resolve_rustbuffer_symbols(
    handle: &LibraryHandle,
    definitions: &JsObject,
) -> Result<RustBufferOps> {
    let symbols: JsObject = definitions.get_named_property("symbols")?;
    let alloc_name: String = symbols
        .get_named_property::<napi::JsString>("rustbufferAlloc")?
        .into_utf8()?
        .as_str()?
        .to_owned();
    let free_name: String = symbols
        .get_named_property::<napi::JsString>("rustbufferFree")?
        .into_utf8()?
        .as_str()?
        .to_owned();
    let from_bytes_name: String = symbols
        .get_named_property::<napi::JsString>("rustbufferFromBytes")?
        .into_utf8()?
        .as_str()?
        .to_owned();

    // We only need from_bytes and free at call time; alloc is resolved to validate it exists.
    let _alloc_ptr = handle.lookup_symbol(&alloc_name)?;
    let free_ptr = handle.lookup_symbol(&free_name)?;
    let from_bytes_ptr = handle.lookup_symbol(&from_bytes_name)?;

    Ok(RustBufferOps {
        from_bytes_ptr,
        free_ptr,
    })
}

/// Build a JS object whose methods call into the native library described by `definitions`.
///
/// See the module-level documentation for the full registration pipeline.
pub fn register(env: Env, handle: &LibraryHandle, definitions: JsObject) -> Result<JsObject> {
    // Resolve rustbuffer symbols from definitions
    let rb_ops = resolve_rustbuffer_symbols(handle, &definitions)?;

    // Parse callback definitions if present
    let callback_defs = parse_callbacks(&definitions)?;
    let callback_defs = Rc::new(callback_defs);

    // Parse struct definitions if present
    let struct_defs = structs::parse_structs(&definitions)?;
    let struct_defs = Rc::new(struct_defs);

    let functions: JsObject = definitions.get_named_property("functions")?;
    let mut result = env.create_object()?;

    let names = functions.get_property_names()?;
    let len = names.get_array_length()?;

    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let func_def: JsObject = functions.get_named_property(&name)?;

        // Parse argument types
        let args_arr: JsObject = func_def.get_named_property("args")?;
        let args_len = args_arr.get_array_length()?;
        let mut arg_types = Vec::with_capacity(args_len as usize);
        for j in 0..args_len {
            let arg_obj: JsObject = args_arr.get_element(j)?;
            arg_types.push(FfiTypeDesc::from_js_object(&arg_obj)?);
        }

        // Parse return type
        let ret_obj: JsObject = func_def.get_named_property("ret")?;
        let ret_type = FfiTypeDesc::from_js_object(&ret_obj)?;

        // Check hasRustCallStatus
        let has_rust_call_status: bool = func_def.get_named_property("hasRustCallStatus")?;

        // Look up symbol
        let symbol_ptr = handle.lookup_symbol(&name)?;

        // Build CIF: declared args + optional RustCallStatus pointer
        let mut cif_arg_types: Vec<libffi::middle::Type> = arg_types
            .iter()
            .map(|t| ffi_type_for(t, &struct_defs))
            .collect::<napi::Result<Vec<_>>>()?;
        if has_rust_call_status {
            cif_arg_types.push(libffi::middle::Type::pointer());
        }
        let cif_ret_type = ffi_type_for(&ret_type, &struct_defs)?;
        let cif = Cif::new(cif_arg_types, cif_ret_type);

        let func = Rc::new(ResolvedFfiFunc {
            cif: Rc::new(cif),
            symbol_ptr,
            arg_types: Rc::new(arg_types),
            ret_type,
            has_rust_call_status,
        });
        let defs = Rc::new(DefinitionContext {
            callbacks: callback_defs.clone(),
            structs: struct_defs.clone(),
        });

        let js_func = env.create_function_from_closure(&name, move |ctx| {
            call_ffi_function(ctx.env, &ctx, &func, &rb_ops, &defs)
        })?;

        result.set_named_property(&name, js_func)?;
    }

    Ok(result)
}

/// Parse the `callbacks` map from JS definitions into a HashMap of CallbackDefs.
fn parse_callbacks(definitions: &JsObject) -> Result<HashMap<String, CallbackDef>> {
    let mut map = HashMap::new();

    // Callbacks is optional.
    let has_callbacks: bool = definitions.has_named_property("callbacks")?;
    if !has_callbacks {
        return Ok(map);
    }

    let callbacks: JsObject = definitions.get_named_property("callbacks")?;
    let names = callbacks.get_property_names()?;
    let len = names.get_array_length()?;

    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let cb_def: JsObject = callbacks.get_named_property(&name)?;

        // Parse args
        let args_arr: JsObject = cb_def.get_named_property("args")?;
        let args_len = args_arr.get_array_length()?;
        let mut args = Vec::with_capacity(args_len as usize);
        for j in 0..args_len {
            let arg_obj: JsObject = args_arr.get_element(j)?;
            args.push(FfiTypeDesc::from_js_object(&arg_obj)?);
        }

        // Parse ret
        let ret_obj: JsObject = cb_def.get_named_property("ret")?;
        let ret = FfiTypeDesc::from_js_object(&ret_obj)?;

        // Parse hasRustCallStatus
        let has_rust_call_status: bool = cb_def.get_named_property("hasRustCallStatus")?;

        // Parse outReturn (optional, defaults to false)
        let out_return: bool = cb_def
            .get_named_property::<bool>("outReturn")
            .unwrap_or(false);

        map.insert(
            name,
            CallbackDef {
                args,
                ret,
                has_rust_call_status,
                out_return,
            },
        );
    }

    Ok(map)
}

/// Execute a single FFI call: marshal JS arguments to C, invoke via libffi, marshal back.
///
/// See the module-level "Call Pipeline" section for a high-level overview. The function
/// is parameterized by everything it needs — symbol pointer, type descriptors, RustBuffer
/// helpers, callback/struct definitions — so it is stateless between calls.
fn call_ffi_function(
    env: &Env,
    ctx: &napi::CallContext<'_>,
    func: &ResolvedFfiFunc,
    rb_ops: &RustBufferOps,
    defs: &DefinitionContext,
) -> Result<JsUnknown> {
    let declared_arg_count = func.arg_types.len();

    // The function pointer values for callback trampolines, stored separately
    // so we can borrow them for ffi args.
    let mut callback_fn_ptrs: Vec<*const c_void> = Vec::new();

    // Storage for struct (VTable) pointers passed by reference.
    let mut struct_ptrs: Vec<*const c_void> = Vec::new();

    // Marshal JS arguments to boxed Rust values
    let mut boxed_args: Vec<Box<dyn Any>> = Vec::with_capacity(declared_arg_count);
    for (i, desc) in func.arg_types.iter().enumerate() {
        let js_val: JsUnknown = ctx.get(i)?;
        match desc {
            FfiTypeDesc::RustBuffer => {
                // Convert Uint8Array -> RustBufferC via rustbuffer_from_bytes
                let rust_buffer = unsafe {
                    napi_utils::js_uint8array_to_rust_buffer(
                        env.raw(),
                        js_val,
                        rb_ops.from_bytes_ptr,
                    )?
                };
                boxed_args.push(Box::new(rust_buffer));
            }
            FfiTypeDesc::Reference(inner) if matches!(inner.as_ref(), FfiTypeDesc::Struct(_)) => {
                // Reference(Struct("Name")) — build a VTable struct from a JS object
                let FfiTypeDesc::Struct(struct_name) = inner.as_ref() else {
                    unreachable!("guard ensures inner is Struct");
                };
                let struct_def = defs.structs.get(struct_name).ok_or_else(|| {
                    napi::Error::from_reason(format!("Unknown struct: {struct_name}"))
                })?;

                let js_obj = unsafe { JsObject::from_raw(env.raw(), js_val.raw())? };

                let struct_ptr = structs::build_vtable_struct(
                    env,
                    struct_def,
                    &js_obj,
                    &defs.callbacks,
                    &defs.structs,
                    rb_ops,
                )?;
                struct_ptrs.push(struct_ptr);

                boxed_args.push(Box::new(()));
            }
            FfiTypeDesc::Callback(cb_name) => {
                let cb_def = defs.callbacks.get(cb_name).ok_or_else(|| {
                    napi::Error::from_reason(format!("Unknown callback: {cb_name}"))
                })?;

                // SAFETY: Same rationale as the JsObject::from_raw above — env and js_val
                // are valid napi handles from the current call context.
                let js_fn = unsafe { napi::JsFunction::from_raw(env.raw(), js_val.raw())? };

                // Create a GC-preventing reference to the JS function, so it survives
                // beyond this scope. Same pattern as VTableTrampolineUserdata in structs.rs.
                let mut fn_ref: napi::sys::napi_ref = std::ptr::null_mut();
                // SAFETY: `env.raw()` is the current valid env, `js_val.raw()` is a
                // live napi_value. The initial refcount of 1 keeps the JS function
                // alive for the process lifetime.
                let ref_status = unsafe {
                    napi::sys::napi_create_reference(env.raw(), js_val.raw(), 1, &mut fn_ref)
                };
                if ref_status != napi::sys::Status::napi_ok {
                    return Err(napi::Error::from_reason(format!(
                        "Failed to create reference for callback '{cb_name}'"
                    )));
                }

                // Create a ThreadsafeFunction for cross-thread dispatch.
                // The callback converts RawCallbackArg values to JS values on the main thread.
                let tsfn: napi::threadsafe_function::ThreadsafeFunction<
                    Vec<RawCallbackArg>,
                    napi::threadsafe_function::ErrorStrategy::Fatal,
                > = js_fn.create_threadsafe_function(
                    0,
                    |ctx: napi::threadsafe_function::ThreadSafeCallContext<Vec<RawCallbackArg>>| {
                        let mut js_args: Vec<napi::JsUnknown> = Vec::with_capacity(ctx.value.len());
                        for raw_arg in &ctx.value {
                            js_args.push(raw_arg_to_js(&ctx.env, raw_arg)?);
                        }
                        Ok(js_args)
                    },
                )?;

                // Unref the ThreadsafeFunction so it doesn't keep the Node.js event loop alive.
                // The ThreadsafeFunction will still work when called, but won't prevent process exit.
                let mut tsfn = tsfn;
                tsfn.unref(env)?;

                // Create userdata on the heap with a stable address
                let userdata = Box::new(TrampolineUserdata {
                    raw_env: env.raw(),
                    fn_ref,
                    arg_types: cb_def.args.clone(),
                    tsfn: Some(tsfn),
                    rb_free_ptr: rb_ops.free_ptr,
                });

                // Build the callback CIF
                let cb_cif = callback::build_callback_cif(cb_def, &defs.structs)?;

                // Leak the userdata so it survives beyond this function call.
                // This is necessary because the callback may be invoked from another
                // thread after call_ffi_function returns.
                let userdata_ptr = Box::into_raw(userdata);
                // SAFETY: `userdata_ptr` was just returned by `Box::into_raw`, so it
                // points to a valid, fully-initialized `TrampolineUserdata` on the heap.
                // By leaking the Box we guarantee the allocation is never freed, making
                // the `&'static` borrow sound. The trampoline closure (below) and any
                // cross-thread callback invocations may dereference this pointer at any
                // future time; the intentional leak ensures it remains valid.
                let userdata_ref: &'static TrampolineUserdata = unsafe { &*userdata_ptr };

                // Create the closure with 'static lifetime since userdata is leaked.
                let closure = Closure::new(cb_cif, callback::trampoline_callback, userdata_ref);

                // Extract the function pointer value from the closure.
                // SAFETY: `closure.code_ptr()` returns a `CodePtr` wrapping the executable
                // trampoline that libffi JIT-compiled for this closure. Dereferencing it
                // yields the raw function pointer, which we cast to `*const c_void` for
                // storage. The pointer remains valid as long as the closure is alive — and
                // we ensure that by `mem::forget`-ing the closure immediately below.
                let fn_ptr: *const c_void = *closure.code_ptr() as *const std::ffi::c_void;
                callback_fn_ptrs.push(fn_ptr);

                // SAFETY: We intentionally leak the closure so that `fn_ptr` (derived from
                // its code pointer) remains valid for the lifetime of the process. Dropping
                // the closure would deallocate the libffi trampoline, leaving `fn_ptr` as a
                // dangling pointer. Because the native library may invoke the callback from
                // any thread at any future time, we cannot determine a safe point to drop it.
                // This is the same "intentional leak" pattern used for `userdata` above.
                std::mem::forget(closure);

                // Placeholder for boxed_args (not used for callbacks)
                boxed_args.push(Box::new(()));
            }
            _ => {
                boxed_args.push(marshal::js_to_boxed(env, desc, js_val)?);
            }
        }
    }

    // Build libffi Arg references
    let mut ffi_args: Vec<Arg> = Vec::with_capacity(declared_arg_count + 1);
    let mut cb_ptr_idx = 0;
    let mut struct_ptr_idx = 0;
    for (i, desc) in func.arg_types.iter().enumerate() {
        match desc {
            FfiTypeDesc::RustBuffer => {
                let rb = boxed_args[i].downcast_ref::<RustBufferC>().ok_or_else(|| {
                    napi::Error::from_reason(
                        "Type mismatch: expected RustBufferC for RustBuffer arg",
                    )
                })?;
                ffi_args.push(arg(rb));
            }
            FfiTypeDesc::Callback(_) => {
                ffi_args.push(arg(&callback_fn_ptrs[cb_ptr_idx]));
                cb_ptr_idx += 1;
            }
            FfiTypeDesc::Reference(inner) if matches!(inner.as_ref(), FfiTypeDesc::Struct(_)) => {
                ffi_args.push(arg(&struct_ptrs[struct_ptr_idx]));
                struct_ptr_idx += 1;
            }
            _ => {
                ffi_args.push(marshal::boxed_to_arg(desc, boxed_args[i].as_ref())?);
            }
        }
    }

    // Handle RustCallStatus
    let mut rust_call_status = RustCallStatusC::default();
    let status_ptr: *mut RustCallStatusC;
    let mut status_js_obj: Option<JsObject> = None;

    if func.has_rust_call_status {
        // The last JS argument is the status object { code: number }
        let status_idx = declared_arg_count;
        let js_status: JsObject = ctx.get(status_idx)?;
        let code_val: i32 = js_status.get_named_property("code")?;
        rust_call_status.code = code_val as i8;
        status_js_obj = Some(js_status);

        // Pass pointer to rust_call_status as the last C arg
        status_ptr = &mut rust_call_status as *mut RustCallStatusC;
        ffi_args.push(arg(&status_ptr));
    }

    // Call the function
    let code_ptr = CodePtr::from_ptr(func.symbol_ptr as *mut c_void);
    let ret_val: Box<dyn Any> = call_with_ret_type(&func.cif, code_ptr, &ffi_args, &func.ret_type)?;

    // Write back RustCallStatus
    if func.has_rust_call_status {
        if let Some(mut js_status) = status_js_obj {
            js_status
                .set_named_property("code", env.create_int32(rust_call_status.code as i32)?)?;

            // If the call reported an error, copy the error buffer into a JS Uint8Array
            // and attach it to the status object, then free the native error buffer.
            // SAFETY (ordering): `create_uint8array` copies from `error_buf_data` into a
            // new JS ArrayBuffer, so we must call it *before* `free_rustbuffer` releases
            // the native allocation. This is the same create-then-free pattern used in
            // `rust_buffer_to_js_uint8array`.
            if rust_call_status.code != 0 && !rust_call_status.error_buf_data.is_null() {
                let raw_env = env.raw();

                // Build the RustBufferC for the error buffer up front so we can
                // free it in all paths (success and failure).
                let error_rb = RustBufferC {
                    capacity: rust_call_status.error_buf_capacity,
                    len: rust_call_status.error_buf_len,
                    data: rust_call_status.error_buf_data,
                };

                match usize::try_from(rust_call_status.error_buf_len) {
                    Ok(len) => {
                        if let Ok(typedarray) = unsafe {
                            napi_utils::create_uint8array(
                                raw_env,
                                rust_call_status.error_buf_data,
                                len,
                            )
                        } {
                            if let Ok(js_uint8array) =
                                unsafe { JsUnknown::from_raw(raw_env, typedarray) }
                            {
                                js_status.set_named_property("errorBuf", js_uint8array)?;
                            } else {
                                #[cfg(debug_assertions)]
                                eprintln!(
                                    "uniffi-runtime-napi: failed to wrap error buffer as JsUnknown"
                                );
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "uniffi-runtime-napi: failed to create Uint8Array for error buffer ({len} bytes)"
                            );
                        }
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "uniffi-runtime-napi: error buffer len {} exceeds addressable memory",
                            rust_call_status.error_buf_len
                        );
                    }
                }

                // Free the error_buf via rustbuffer_free
                unsafe { napi_utils::free_rustbuffer(error_rb, rb_ops.free_ptr) };
            }
        }
    }

    // Marshal return value to JS
    match &func.ret_type {
        FfiTypeDesc::RustBuffer => {
            let rb = ret_val.downcast_ref::<RustBufferC>().ok_or_else(|| {
                napi::Error::from_reason(
                    "Type mismatch: expected RustBufferC for RustBuffer return",
                )
            })?;
            rust_buffer_to_js_uint8array(env, *rb, rb_ops.free_ptr)
        }
        _ => marshal::ret_to_js(env, &func.ret_type, ret_val.as_ref()),
    }
}

/// Dispatch `cif.call` with the correct Rust return type inferred from `ret_type`.
///
/// libffi's `Cif::call` is generic over the return type `R`, so we must monomorphize
/// on every supported scalar/buffer type. The result is type-erased into a
/// `Box<dyn Any>` that `ret_to_js` (or `rust_buffer_to_js_uint8array`) later downcasts.
fn call_with_ret_type(
    cif: &Cif,
    code_ptr: CodePtr,
    args: &[Arg],
    ret_type: &FfiTypeDesc,
) -> Result<Box<dyn Any>> {
    // SAFETY: The entire body is one `unsafe` block because `cif.call` is an unsafe
    // function. The safety contract requires:
    //
    // 1. **Type agreement.** The CIF was built from the same `FfiTypeDesc` list that
    //    drove argument marshalling in `call_ffi_function`, so the libffi type
    //    descriptors, the concrete types of the `Arg` values, and the monomorphized
    //    return type `R` all agree.
    // 2. **Argument count.** `args` was assembled in `call_ffi_function` with exactly
    //    one entry per declared argument plus an optional `RustCallStatus` pointer —
    //    matching the CIF's argument count.
    // 3. **Valid code pointer.** `code_ptr` was obtained via `dlsym` on the loaded
    //    native library (see `LibraryHandle::lookup_symbol`), and the library remains
    //    loaded for the lifetime of this process.
    unsafe {
        match ret_type {
            FfiTypeDesc::Void => {
                cif.call::<()>(code_ptr, args);
                Ok(Box::new(()))
            }
            FfiTypeDesc::UInt8 => {
                let r: u8 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Int8 => {
                let r: i8 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::UInt16 => {
                let r: u16 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Int16 => {
                let r: i16 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::UInt32 => {
                let r: u32 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Int32 => {
                let r: i32 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
                let r: u64 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Int64 => {
                let r: i64 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Float32 => {
                let r: f32 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::Float64 => {
                let r: f64 = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            FfiTypeDesc::RustBuffer => {
                let r: RustBufferC = cif.call(code_ptr, args);
                Ok(Box::new(r))
            }
            _ => Err(napi::Error::from_reason(format!(
                "Unsupported return type: {ret_type:?}"
            ))),
        }
    }
}

/// Convert a `RustBufferC` to a JS `Uint8Array`, then free the native buffer.
///
/// **Ordering matters.** We must create the JS typed array (which copies the bytes into
/// a V8-managed `ArrayBuffer`) *before* freeing the `RustBufferC`. If we freed first,
/// `rb.data` would be a dangling pointer during the copy.
fn rust_buffer_to_js_uint8array(
    env: &Env,
    rb: RustBufferC,
    rb_free_ptr: *const c_void,
) -> Result<JsUnknown> {
    let len = usize::try_from(rb.len).map_err(|_| {
        unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };
        napi::Error::from_reason("RustBuffer len exceeds addressable memory")
    })?;
    let raw_env = env.raw();
    // SAFETY: `rb.data` points to a valid allocation of at least `len` bytes,
    // owned by the Rust allocator via the native library. `create_uint8array` copies
    // the data into a new JS ArrayBuffer, so after this call we no longer need the
    // original allocation.
    let typedarray = unsafe { napi_utils::create_uint8array(raw_env, rb.data, len)? };

    // Free the RustBuffer now that we have copied the data into JS-managed memory.
    // SAFETY: `rb` is a valid RustBufferC returned by the native library, and
    // `rb_free_ptr` is the corresponding `rustbuffer_free` symbol.
    unsafe { napi_utils::free_rustbuffer(rb, rb_free_ptr) };

    Ok(unsafe { JsUnknown::from_raw(raw_env, typedarray)? })
}
