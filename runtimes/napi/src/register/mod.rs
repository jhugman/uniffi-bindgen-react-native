/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Registration: parse JS definitions, open the library, and build JS closures
//! that dispatch to the per-call path in `call.rs`.

mod spec_from_js;

use std::ffi::c_void;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::{JsObject, JsUnknown, NapiRaw, NapiValue, Result};

use crate::call::call_ffi_function;
use crate::core_err;
use crate::napi_utils;
use crate::napi_utils::CapacitySymbol;
use uniffi_runtime_core::ffi_c_types::RustBufferC;
use uniffi_runtime_core::{FfiTypeDesc, Module};

/// Build a JS object whose methods call into the native library described by `definitions`.
pub fn register(
    env: Env,
    library_path: &str,
    definitions: JsObject,
) -> Result<(JsObject, Arc<Module>)> {
    let spec = spec_from_js::parse_module_spec(&definitions)?;

    extern "C" fn noop_abort(_: *const c_void) {}
    let module = Module::new(Path::new(library_path), spec, noop_abort, std::ptr::null())
        .map_err(core_err)?;

    let functions: JsObject = definitions.get_named_property("functions")?;
    let mut result = env.create_object()?;

    // Per-registration Symbol used as hidden ownership metadata on RustBuffer
    // views. Rust-owned lift views carry their allocation capacity; JS-owned
    // views carry zero so free is a no-op.
    //
    // SAFETY: env is the active napi env supplied by node for this register
    // call. The `Arc<CapacitySymbol>` keeps the Symbol alive across the
    // module facade's lifetime (closures captured below outlive the call).
    let capacity_symbol = Arc::new(unsafe { CapacitySymbol::new(env.raw())? });

    let names = functions.get_property_names()?;
    let len = names.get_array_length()?;

    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();

        let fn_name = name.clone();
        let module_ref = Arc::clone(&module);

        let func_def = module.function_def(&name).ok_or_else(|| {
            napi::Error::from_reason(format!("Function not found in module: {name}"))
        })?;
        let arg_types: Rc<Vec<FfiTypeDesc>> = Rc::new(func_def.args.clone());
        let has_rust_call_status = func_def.has_rust_call_status;
        let cap_sym_for_call = Arc::clone(&capacity_symbol);

        let js_func = env.create_function_from_closure(&name, move |ctx| {
            call_ffi_function(
                ctx.env,
                &ctx,
                &fn_name,
                &module_ref,
                &arg_types,
                has_rust_call_status,
                &cap_sym_for_call,
            )
        })?;

        result.set_named_property(&name, js_func)?;
    }

    // Lowering serializes into a JS-owned buffer. Call dispatch then copies those bytes
    // into the Rust-owned buffer that the FFI function consumes. Mark the view as
    // JS-owned so an explicit `rustbuffer_free(view)` is also a safe no-op.
    let cap_sym_for_alloc = Arc::clone(&capacity_symbol);
    let alloc_fn = env.create_function_from_closure("rustbuffer_alloc", move |ctx| {
        let size_arg: i32 = ctx.get(0)?;
        if size_arg < 0 {
            return Err(napi::Error::from_reason(
                "rustbuffer_alloc size must be non-negative".to_string(),
            ));
        }
        let raw_env = ctx.env.raw();
        let typedarray =
            unsafe { napi_utils::create_uint8array(raw_env, std::ptr::null(), size_arg as usize)? };
        unsafe { cap_sym_for_alloc.set(raw_env, typedarray, 0)? };
        unsafe { JsUnknown::from_raw(raw_env, typedarray) }
    })?;
    result.set_named_property("rustbuffer_alloc", alloc_fn)?;

    let free_module = Arc::clone(&module);
    let cap_sym_for_free = Arc::clone(&capacity_symbol);
    let free_fn = env.create_function_from_closure("rustbuffer_free", move |ctx| {
        let js_val: JsUnknown = ctx.get(0)?;
        let raw_env = ctx.env.raw();
        // SAFETY: js_val is a JS value from the current callback scope; `raw()`
        // returns the underlying napi_value handle without changing its ownership.
        let raw_val = unsafe { js_val.raw() };
        // SAFETY: js_val came from JS; if it is not a typed array,
        // `read_typedarray_data` returns None and we surface a clean error.
        let (data_ptr, _) = unsafe { napi_utils::read_typedarray_data(raw_env, raw_val) }
            .ok_or_else(|| {
                napi::Error::from_reason(
                    "rustbuffer_free expected a Uint8Array argument".to_string(),
                )
            })?;
        // Rust-owned lift views carry their allocation capacity. JS-owned
        // lowering buffers and copied lift fallbacks carry zero. An unmarked
        // view did not come from this runtime and must never reach Rust's allocator.
        // SAFETY: raw_env / raw_val are valid for the current callback scope.
        let capacity = unsafe { cap_sym_for_free.get(raw_env, raw_val)? }.ok_or_else(|| {
            napi::Error::from_reason("rustbuffer_free received an unowned Uint8Array")
        })?;
        if capacity == 0 {
            return ctx.env.get_undefined().map(|u| u.into_unknown());
        }

        // Mark the view released before handing its original allocation back to
        // Rust. Repeated cleanup is then a no-op instead of a double free.
        unsafe { cap_sym_for_free.set(raw_env, raw_val, 0)? };
        let rb = RustBufferC {
            capacity,
            len: 0,
            data: data_ptr as *mut u8,
        };
        // SAFETY: free_ptr was resolved at registration time, and the ownership
        // metadata ties this exact pointer and capacity to a Rust lift handoff.
        unsafe { napi_utils::free_rustbuffer(rb, free_module.rb_ops().free_ptr) };
        ctx.env.get_undefined().map(|u| u.into_unknown())
    })?;
    result.set_named_property("rustbuffer_free", free_fn)?;

    Ok((result, module))
}
