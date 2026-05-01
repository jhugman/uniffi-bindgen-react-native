/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! VTable construction: build C-compatible VTable structs using core's APIs.
//!
//! Each VTable field is a Callback-typed function pointer. For each field, we
//! create a `CallbackUserData` and call `Module::make_callback_trampoline` to
//! get a libffi closure, then `Module::build_vtable` assembles them into a
//! leaked C struct.

use std::ffi::c_void;
use std::sync::Arc;

use napi::{Env, JsObject, Result};
use uniffi_runtime_core::{FfiTypeDesc, Module, VTableField};

use crate::callback::{
    create_callback_user_data, dispatch_to_js_thread, is_js_thread, on_js_thread,
};

/// Build a C-compatible VTable struct from a JS object implementing a UniFFI trait.
///
/// For each field in the struct definition:
/// 1. Verify it is a `Callback(name)` type.
/// 2. Extract the JS function from the JS object.
/// 3. Create a `CallbackUserData` and get a trampoline fn pointer from core.
/// 4. Collect as a `VTableField`.
///
/// Finally, call `module.build_vtable(struct_name, &fields)` to assemble the
/// VTable struct. The returned pointer is leaked (process lifetime).
pub fn build_vtable_struct(
    env: &Env,
    module: &Arc<Module>,
    struct_name: &str,
    js_obj: &JsObject,
) -> Result<*const c_void> {
    let struct_def = module
        .spec_structs()
        .get(struct_name)
        .ok_or_else(|| napi::Error::from_reason(format!("unknown struct: {struct_name}")))?;

    let mut vtable_fields = Vec::with_capacity(struct_def.fields.len());

    for field in &struct_def.fields {
        let cb_name = match &field.field_type {
            FfiTypeDesc::Callback(name) => name,
            other => {
                return Err(napi::Error::from_reason(format!(
                    "VTable field '{}' expected Callback, got {other:?}",
                    field.name
                )))
            }
        };

        let js_fn: napi::JsFunction = js_obj.get_named_property(&field.name)?;
        let user_data = create_callback_user_data(env, js_fn, cb_name, module)?;

        let fn_ptr = module
            .make_callback_trampoline(
                cb_name,
                on_js_thread,
                dispatch_to_js_thread,
                is_js_thread,
                user_data,
            )
            .map_err(crate::core_err)?;

        vtable_fields.push(VTableField {
            callback_name: cb_name.clone(),
            fn_ptr,
        });
    }

    module
        .build_vtable(struct_name, &vtable_fields)
        .map_err(crate::core_err)
}
