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

    // Per-registration Symbol used as a hidden capacity-hint key on lift-
    // handoff `Uint8Array` views. The view-handoff path returns a view whose
    // `byteLength` is `rb.len`, but Rust may have allocated
    // `rb.capacity > rb.len`. We stash `capacity` on the view via this
    // symbol; `rustbuffer_free(view)` reads it back when releasing the
    // allocation.
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

    // `rustbuffer_alloc(n)` -> Uint8Array view over Rust-owned memory of capacity `n`.
    // `rustbuffer_free(view)` -> hands the underlying (ptr, capacity) back to the
    // library's `rustbuffer_free`. Together they let JS allocate buffers that the
    // codegen-emitted lowering path can fill in place and ship to Rust without copying.
    let alloc_module = Arc::clone(&module);
    let cap_sym_for_alloc = Arc::clone(&capacity_symbol);
    let alloc_fn = env.create_function_from_closure("rustbuffer_alloc", move |ctx| {
        let size_arg: i32 = ctx.get(0)?;
        if size_arg < 0 {
            return Err(napi::Error::from_reason(
                "rustbuffer_alloc size must be non-negative".to_string(),
            ));
        }
        // SAFETY: `alloc_ptr` was resolved at registration time via dlsym; module
        // (and thus the loaded library) outlives this closure thanks to the captured Arc.
        let rb =
            unsafe { napi_utils::rustbuffer_alloc(size_arg, alloc_module.rb_ops().alloc_ptr)? };
        let len = usize::try_from(rb.capacity).map_err(|_| {
            // Free what we just allocated to avoid leaking on the error path.
            unsafe { napi_utils::free_rustbuffer(rb, alloc_module.rb_ops().free_ptr) };
            napi::Error::from_reason("RustBuffer capacity exceeds addressable memory".to_string())
        })?;
        // SAFETY: rb.data points to a valid allocation of `len` bytes that the Rust
        // library owns; the view is released either by `rustbuffer_free(view)` or by being
        // adopted when passed as an FFI argument, so no finalizer is required.
        //
        // CONTRACT: adoption frees the backing allocation but cannot detach this view, so the
        // `Uint8Array` is left dangling over freed memory. A view must be treated as consumed
        // once it has been passed as an FFI argument: reading it afterwards is a use-after-free,
        // and passing it a second time fails with an "already consumed" error. Generated lowering
        // code drops the view immediately, so this only bites hand-written callers.
        let typedarray =
            unsafe { napi_utils::create_external_uint8array(ctx.env.raw(), rb.data, len)? };
        // Stamp the capacity so the runtime can recognise this view as library-owned. Two
        // consumers rely on it: `rustbuffer_free` frees against the true capacity, and the
        // argument-lowering path adopts the allocation instead of copying it. Without the
        // stamp, a lowered argument gets copied and this allocation is orphaned.
        if rb.capacity > 0 {
            unsafe { cap_sym_for_alloc.set(ctx.env.raw(), typedarray, rb.capacity)? };
        }
        unsafe { JsUnknown::from_raw(ctx.env.raw(), typedarray) }
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
        let (data_ptr, length) = unsafe { napi_utils::read_typedarray_data(raw_env, raw_val) }
            .ok_or_else(|| {
                napi::Error::from_reason(
                    "rustbuffer_free expected a Uint8Array argument".to_string(),
                )
            })?;
        // Empty views never carry a capacity hint (view-handoff short-
        // circuits empty buffers, and `rustbuffer_alloc(0)` is itself a
        // no-op). Bail out before the napi_ref + napi_has_property dance.
        if length == 0 {
            return ctx.env.get_undefined().map(|u| u.into_unknown());
        }
        // Capacity recovery. Three cases, all keyed off the hint:
        //   (a) `rustbuffer_alloc(n)` views: hint == byteLength == n.
        //   (b) Lift-handoff views: byteLength == rb.len, and the hint carries the true
        //       capacity, which may be larger. Set at handoff time when capacity != len.
        //   (c) Hint == 0: the view was already adopted as an FFI argument, so the callee
        //       has freed the memory. `free_rustbuffer` skips zero-capacity buffers, which
        //       turns this into the no-op it must be rather than a double free.
        // No hint at all means the memory is not the library's to free; fall back to
        // byteLength to preserve the historical behaviour for such views.
        // SAFETY: raw_env / raw_val are valid for the current callback scope.
        let capacity = unsafe { cap_sym_for_free.get(raw_env, raw_val) }.unwrap_or(length as u64);
        let rb = RustBufferC {
            capacity,
            len: 0,
            data: data_ptr as *mut u8,
        };
        // SAFETY: free_ptr was resolved at registration time. rb mirrors the
        // buffer produced by the matching `rustbuffer_alloc` call OR a
        // lift-handoff view whose `(data_ptr, capacity)` match the original
        // RustBuffer that Rust returned across the FFI.
        unsafe { napi_utils::free_rustbuffer(rb, free_module.rb_ops().free_ptr) };
        ctx.env.get_undefined().map(|u| u.into_unknown())
    })?;
    result.set_named_property("rustbuffer_free", free_fn)?;

    Ok((result, module))
}
