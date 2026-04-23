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
use napi::{JsObject, Result};

use crate::call::call_ffi_function;
use crate::core_err;
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

        let js_func = env.create_function_from_closure(&name, move |ctx| {
            call_ffi_function(
                ctx.env,
                &ctx,
                &fn_name,
                &module_ref,
                &arg_types,
                has_rust_call_status,
            )
        })?;

        result.set_named_property(&name, js_func)?;
    }

    Ok((result, module))
}
