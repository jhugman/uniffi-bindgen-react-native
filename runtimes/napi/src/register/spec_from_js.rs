/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Parse a `definitions` JsObject from JS into a plain-Rust `ModuleSpec`.

use std::collections::HashMap;

use napi::{JsObject, Result};
use uniffi_runtime_core::{
    desc_from_name, CallbackDef, FfiTypeDesc, FunctionDef, ModuleSpec, RustBufferSymbols,
    StructDef, StructField,
};

/// Parse an `FfiTypeDesc` from a JS object with shape `{ tag: string, ...params }`.
fn ffi_type_desc_from_js(obj: &JsObject) -> Result<FfiTypeDesc> {
    let tag: String = obj.get_named_property::<String>("tag")?;
    match tag.as_str() {
        // No wire tag, so not on core's table: napi resolves these itself.
        "ForeignBytes" => Ok(FfiTypeDesc::ForeignBytes),
        "VoidPointer" => Ok(FfiTypeDesc::VoidPointer),
        // A reference's target is another type object, so it recurses rather
        // than resolving through a tag (the wire tag only ever wraps a Struct).
        "Reference" => {
            let inner: JsObject = obj.get_named_property("inner")?;
            Ok(FfiTypeDesc::Reference(Box::new(ffi_type_desc_from_js(
                &inner,
            )?)))
        }
        "MutReference" => {
            let inner: JsObject = obj.get_named_property("inner")?;
            Ok(FfiTypeDesc::MutReference(Box::new(ffi_type_desc_from_js(
                &inner,
            )?)))
        }
        other => {
            // Callback and Struct take their name from the same object; core
            // rejects them without one.
            let name: Option<String> = match other {
                "Callback" | "Struct" => Some(obj.get_named_property::<String>("name")?),
                _ => None,
            };
            desc_from_name(other, name.as_deref())
                .map_err(|e| napi::Error::from_reason(e.to_string()))
        }
    }
}

pub fn parse_module_spec(definitions: &JsObject) -> Result<ModuleSpec> {
    let symbols: JsObject = definitions.get_named_property("symbols")?;
    let rustbuffer_symbols = RustBufferSymbols {
        alloc: symbols.get_named_property::<String>("rustbuffer_alloc")?,
        free: symbols.get_named_property::<String>("rustbuffer_free")?,
        from_bytes: symbols.get_named_property::<String>("rustbuffer_from_bytes")?,
    };

    let functions = parse_functions(definitions)?;
    let callbacks = parse_callbacks(definitions)?;
    let structs = parse_structs(definitions)?;

    Ok(ModuleSpec {
        rustbuffer_symbols,
        functions,
        callbacks,
        structs,
    })
}

fn parse_functions(defs: &JsObject) -> Result<HashMap<String, FunctionDef>> {
    let mut out = HashMap::new();
    let functions: JsObject = defs.get_named_property("functions")?;
    let names = functions.get_property_names()?;
    let len = names.get_array_length()?;
    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let f: JsObject = functions.get_named_property(&name)?;
        let args_arr: JsObject = f.get_named_property("args")?;
        let args_len = args_arr.get_array_length()?;
        let mut args = Vec::with_capacity(args_len as usize);
        for j in 0..args_len {
            let a: JsObject = args_arr.get_element(j)?;
            args.push(ffi_type_desc_from_js(&a)?);
        }
        let ret_obj: JsObject = f.get_named_property("ret")?;
        let ret = ffi_type_desc_from_js(&ret_obj)?;
        let has_rust_call_status: bool = f.get_named_property("hasRustCallStatus")?;
        out.insert(
            name,
            FunctionDef {
                args,
                ret,
                has_rust_call_status,
            },
        );
    }
    Ok(out)
}

fn parse_callbacks(defs: &JsObject) -> Result<HashMap<String, CallbackDef>> {
    let mut out = HashMap::new();
    if !defs.has_named_property("callbacks")? {
        return Ok(out);
    }
    let callbacks: JsObject = defs.get_named_property("callbacks")?;
    let names = callbacks.get_property_names()?;
    let len = names.get_array_length()?;
    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let c: JsObject = callbacks.get_named_property(&name)?;
        let args_arr: JsObject = c.get_named_property("args")?;
        let args_len = args_arr.get_array_length()?;
        let mut args = Vec::with_capacity(args_len as usize);
        for j in 0..args_len {
            let a: JsObject = args_arr.get_element(j)?;
            args.push(ffi_type_desc_from_js(&a)?);
        }
        let ret_obj: JsObject = c.get_named_property("ret")?;
        let ret = ffi_type_desc_from_js(&ret_obj)?;
        let has_rust_call_status: bool = c.get_named_property("hasRustCallStatus")?;
        let out_return: bool = c.get_named_property::<bool>("outReturn").unwrap_or(false);
        out.insert(
            name,
            CallbackDef {
                args,
                ret,
                has_rust_call_status,
                out_return,
            },
        );
    }
    Ok(out)
}

fn parse_structs(defs: &JsObject) -> Result<HashMap<String, StructDef>> {
    let mut out = HashMap::new();
    if !defs.has_named_property("structs")? {
        return Ok(out);
    }
    let structs: JsObject = defs.get_named_property("structs")?;
    let names = structs.get_property_names()?;
    let len = names.get_array_length()?;
    for i in 0..len {
        let name: String = names
            .get_element::<napi::JsString>(i)?
            .into_utf8()?
            .as_str()?
            .to_owned();
        // The JS shape is an array of { name, type } objects directly:
        //   { TestVTable: [ { name: "get_value", type: { tag: "Callback", name: "..." } }, ... ] }
        let fields_arr: JsObject = structs.get_named_property(&name)?;
        let fields_len = fields_arr.get_array_length()?;
        let mut fields = Vec::with_capacity(fields_len as usize);
        for j in 0..fields_len {
            let f: JsObject = fields_arr.get_element(j)?;
            let field_name: String = f.get_named_property("name")?;
            let type_obj: JsObject = f.get_named_property("type")?;
            let field_type = ffi_type_desc_from_js(&type_obj)?;
            fields.push(StructField {
                name: field_name,
                field_type,
            });
        }
        out.insert(name, StructDef { fields });
    }
    Ok(out)
}
