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
use napi::{JsObject, JsUnknown, Result};

use crate::buffers::{self, BufferViews};
use crate::call::call_ffi_function;
use crate::core_err;
use crate::napi_utils::CapacitySymbol;
use uniffi_runtime_core::{FfiTypeDesc, Module};

/// State created once per `register()` call and shared by every function closure the
/// resulting facade exposes.
///
/// The Symbols are keyed to the `napi_env` that `register()` ran on, which is what makes
/// their contents safe to reuse across calls. Bundling them keeps the dispatch signature
/// manageable as more per-registration state accrues.
pub(crate) struct Registration {
    /// Hidden capacity-hint key on lift-handoff `Uint8Array` views. A handed-off view's
    /// `byteLength` is `rb.len`, but Rust may have allocated `rb.capacity > rb.len`, so
    /// the capacity is stashed here for `rustbuffer_free(view)` to read back.
    pub(crate) capacity_symbol: CapacitySymbol,
    /// Hidden keys stashing the identity minted for each JS function, used to look
    /// up a built callback trampoline in core's trampoline map. Without this,
    /// dispatch rebuilds one per call — a permanent ~3 KB leak, and the async poll
    /// loop passes its continuation on every poll. See `callback::cache`.
    pub(crate) trampolines: crate::callback::cache::TrampolineCache,
    /// Whether JS may view Rust memory directly, or must be given copies, as under
    /// Electron's memory cage.
    pub(crate) buffer_views: BufferViews,
}

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

    // SAFETY: env is the active napi env supplied by node for this register call. The
    // `Arc<Registration>` keeps both Symbols alive across the module facade's lifetime
    // (the closures captured below outlive this call).
    let registration = Arc::new(Registration {
        // SAFETY: env is the active napi env supplied by node for this register call.
        capacity_symbol: unsafe { CapacitySymbol::new(env.raw())? },
        // SAFETY: as above.
        trampolines: unsafe {
            crate::callback::cache::TrampolineCache::new(env.raw(), module.spec_callbacks().keys())?
        },
        // SAFETY: as above.
        buffer_views: unsafe { BufferViews::probe(env.raw())? },
    });

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
        // Resolved here rather than per call: the return width never changes,
        // and looking it up in the closure would re-hash the function name on
        // every invocation. Sized from `return_size`, the width core's call
        // path writes, not from the arg-slot geometry: a pointer return is
        // written as 8 bytes whatever the host's pointer size.
        let ret_desc: Rc<FfiTypeDesc> = Rc::new(func_def.ret.clone());
        let ret_size = uniffi_runtime_core::return_size(&ret_desc).map_err(core_err)?;
        let reg_for_call = Arc::clone(&registration);

        let js_func = env.create_function_from_closure(&name, move |ctx| {
            call_ffi_function(
                ctx.env,
                &ctx,
                &fn_name,
                &module_ref,
                &arg_types,
                &ret_desc,
                ret_size,
                has_rust_call_status,
                &reg_for_call,
            )
        })?;

        result.set_named_property(&name, js_func)?;
    }

    // `rustbuffer_alloc(n)` and `rustbuffer_free(view)` let JS allocate buffers that the
    // codegen-emitted lowering path fills in place and ships to Rust. Which `rustbuffer_alloc`
    // the facade gets is decided here, once.
    let alloc_fn = match registration.buffer_views {
        BufferViews::Alias => {
            let module = Arc::clone(&module);
            let registration = Arc::clone(&registration);
            env.create_function_from_closure("rustbuffer_alloc", move |ctx| {
                let size: i32 = ctx.get(0)?;
                // SAFETY: `ctx.env` is the active env for this callback.
                unsafe {
                    buffers::alloc_aliased(
                        ctx.env.raw(),
                        &module,
                        size,
                        &registration.capacity_symbol,
                    )
                }
            })?
        }
        BufferViews::Copy => {
            let registration = Arc::clone(&registration);
            env.create_function_from_closure("rustbuffer_alloc", move |ctx| {
                let size: i32 = ctx.get(0)?;
                // SAFETY: `ctx.env` is the active env for this callback.
                unsafe { buffers::alloc_copied(ctx.env.raw(), size, &registration.capacity_symbol) }
            })?
        }
    };
    result.set_named_property("rustbuffer_alloc", alloc_fn)?;

    let free_module = Arc::clone(&module);
    let reg_for_free = Arc::clone(&registration);
    let free_fn = env.create_function_from_closure("rustbuffer_free", move |ctx| {
        let view: JsUnknown = ctx.get(0)?;
        // SAFETY: `ctx.env` is the active env for this callback, and `free_ptr` was resolved
        // when the module opened.
        unsafe {
            buffers::free(
                ctx.env.raw(),
                view,
                free_module.rb_ops().free_ptr,
                &reg_for_free.capacity_symbol,
            )?
        };
        ctx.env.get_undefined().map(|u| u.into_unknown())
    })?;
    result.set_named_property("rustbuffer_free", free_fn)?;

    // A diagnostic, not part of the FFI surface: how many callback trampolines
    // this module has built. Trampolines are leaked by design, so reuse is the
    // only thing bounding the total, and nothing else observable moves when
    // reuse breaks — the leak pins the same JS function object every time.
    // Named to match the JSI player's identical hook, and counting the same
    // thing: builds, from core's single build point.
    let count_module = Arc::clone(&module);
    let count_fn = env.create_function_from_closure("$uniffiTrampolineCount", move |ctx| {
        ctx.env
            .create_int64(count_module.trampolines_built() as i64)
    })?;
    result.set_named_property("$uniffiTrampolineCount", count_fn)?;

    Ok((result, module))
}
