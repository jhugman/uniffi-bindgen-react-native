/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::collections::HashMap;

use heck::ToLowerCamelCase;
use uniffi_bindgen::pipeline::general;

use super::nodes::*;
use super::type_mapping::ffi_type_to_player;
use crate::bindings::gen_typescript::config::TsConfig;
use crate::bindings::gen_typescript::ffi_module::type_mapping::ffi_type_to_ts;
use crate::bindings::gen_typescript::ffi_module::{
    namespace_has_async, FfiArgDecl, FfiFunctionDecl, TsFfiModule, RUST_FUTURE_CONTINUATION,
};

/// The player tag for a callback's `uniffi_out_return` argument type.
///
/// A `VoidPointer` out-return is uniffi's encoding for a method that returns
/// nothing, so it emits `Void`: a pointer tag here makes a consumer write a
/// value through the out-return pointer for a method that has none.
/// `Reference`/`MutReference` wrap the real type and are unwrapped.
pub(crate) fn ret_tag_for_out_return(ty: &general::FfiType) -> String {
    let inner = match ty {
        general::FfiType::Reference(t) | general::FfiType::MutReference(t) => t.as_ref(),
        t => t,
    };
    match inner {
        general::FfiType::VoidPointer => "FfiType.Void".into(),
        other => ffi_type_to_player(other),
    }
}

/// What the player's entry for an async function needs when the player runs
/// the poll loop.
struct AsyncEntry {
    def: PlayerAsyncDef,
    /// The TypeScript type the entry resolves with: `None` for a void future.
    ts_ret: Option<String>,
}

/// Each async function's entry, by the name of the export that starts its
/// future. A future's value has the type its `complete` export returns.
fn async_entries<'a>(
    ffi_definitions: impl IntoIterator<Item = &'a general::FfiDefinition>,
) -> HashMap<String, AsyncEntry> {
    let rust_fns: HashMap<&str, &general::FfiFunction> = ffi_definitions
        .into_iter()
        .filter_map(|def| match def {
            general::FfiDefinition::RustFunction(f) => Some((f.name.0.as_str(), f)),
            _ => None,
        })
        .collect();
    rust_fns
        .values()
        .filter_map(|func| {
            let data = func.async_data.as_ref()?;
            let complete = &data.ffi_rust_future_complete.0;
            let completed = rust_fns
                .get(complete.as_str())
                .unwrap_or_else(|| panic!("async_data names `{complete}`, which is not defined"))
                .return_type
                .ty
                .as_ref()
                .map(|rt| &rt.ty);
            let def = PlayerAsyncDef {
                poll: data.ffi_rust_future_poll.0.clone(),
                cancel: data.ffi_rust_future_cancel.0.clone(),
                complete: complete.clone(),
                free: data.ffi_rust_future_free.0.clone(),
                ret: completed
                    .map(ffi_type_to_player)
                    .unwrap_or_else(|| "FfiType.Void".into()),
            };
            let ts_ret = completed.map(ffi_type_to_ts);
            Some((func.name.0.clone(), AsyncEntry { def, ts_ret }))
        })
        .collect()
}

impl PlayerFfiModule {
    pub(crate) fn from_general(
        namespace: &general::Namespace,
        config: &TsConfig,
        flavor: &crate::AbiFlavor,
        crate_name: String,
        lib_resolution: Option<LibResolution>,
    ) -> Self {
        let has_async = namespace_has_async(namespace);
        let low_level_async_polling = flavor.low_level_async_polling();
        let host_source = PlayerHostSource::for_flavor(flavor);
        // When the flavor lacks a JS TextDecoder (Jsi2), the generated string
        // converter routes through the native shim helpers, so they must
        // appear on NativeModuleInterface.
        let installs_string_helpers = !flavor.supports_text_encoder();

        let symbols = Self::build_symbols(namespace);
        // A player running the poll loop reaches the `rust_future_*` family
        // through an async function's `async` field, so the generated code
        // never names it.
        let names_future_family = has_async && !low_level_async_polling;
        let async_entries = if low_level_async_polling {
            async_entries(&namespace.ffi_definitions)
        } else {
            HashMap::new()
        };
        let functions = Self::build_functions(namespace, names_future_family, &async_entries);
        let callbacks = Self::build_callbacks(namespace, low_level_async_polling);
        let structs = Self::build_structs(namespace);

        // Reuse TsFfiModule for the typed interface, but we'll use raw
        // symbol names (no ubrn_ prefix) in the NativeModuleInterface.
        let ts_module = Self::build_typed_module(
            namespace,
            names_future_family,
            installs_string_helpers,
            &async_entries,
        );

        Self {
            strict_type_checking: config.strict_type_checking,
            async_delivery: config.async_delivery,
            crate_name,
            lib_resolution,
            host_source,
            symbols,
            functions,
            callbacks,
            structs,
            typed_functions: ts_module.functions,
            typed_definitions: ts_module.definitions,
        }
    }

    fn build_symbols(namespace: &general::Namespace) -> PlayerSymbols {
        PlayerSymbols {
            rustbuffer_alloc: namespace.ffi_rustbuffer_alloc.0.clone(),
            rustbuffer_free: namespace.ffi_rustbuffer_free.0.clone(),
            rustbuffer_from_bytes: namespace.ffi_rustbuffer_from_bytes.0.clone(),
        }
    }

    fn build_functions(
        namespace: &general::Namespace,
        names_future_family: bool,
        async_entries: &HashMap<String, AsyncEntry>,
    ) -> Vec<PlayerFunctionDef> {
        let mut result = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::RustFunction(func) = def {
                if TsFfiModule::should_include_function(func, names_future_family) {
                    let args: Vec<String> = func
                        .arguments
                        .iter()
                        .map(|arg| ffi_type_to_player(&arg.ty.ty))
                        .collect();

                    let ret = func
                        .return_type
                        .ty
                        .as_ref()
                        .map(|rt| ffi_type_to_player(&rt.ty))
                        .unwrap_or_else(|| "FfiType.Void".into());

                    result.push(PlayerFunctionDef {
                        name: func.name.0.clone(),
                        args,
                        ret,
                        has_rust_call_status: func.has_rust_call_status_arg,
                        async_entry: async_entries.get(&func.name.0).map(|e| e.def.clone()),
                    });
                }
            }
        }

        result
    }

    fn build_callbacks(
        namespace: &general::Namespace,
        low_level_async_polling: bool,
    ) -> Vec<PlayerCallbackDef> {
        let mut result = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::FunctionType(ft) = def {
                // A player that runs the poll loop supplies the continuation
                // itself, so no table entry describes it.
                if low_level_async_polling && ft.name.0 == RUST_FUTURE_CONTINUATION {
                    continue;
                }
                let has_out_return = ft.arguments.iter().any(|a| {
                    a.name == "uniffi_out_return" || a.name == "uniffi_out_dropped_callback"
                });

                let args: Vec<String> = ft
                    .arguments
                    .iter()
                    .filter(|a| {
                        a.name != "uniffi_out_return" && a.name != "uniffi_out_dropped_callback"
                    })
                    .map(|arg| ffi_type_to_player(&arg.ty.ty))
                    .collect();

                let ret = if has_out_return {
                    ft.arguments
                        .iter()
                        .find(|a| {
                            a.name == "uniffi_out_return" || a.name == "uniffi_out_dropped_callback"
                        })
                        .map(|a| ret_tag_for_out_return(&a.ty.ty))
                        .unwrap_or_else(|| "FfiType.Void".into())
                } else {
                    ft.return_type
                        .ty
                        .as_ref()
                        .map(|rt| ffi_type_to_player(&rt.ty))
                        .unwrap_or_else(|| "FfiType.Void".into())
                };

                result.push(PlayerCallbackDef {
                    name: ft.name.0.clone(),
                    args,
                    ret,
                    has_rust_call_status: ft.has_rust_call_status_arg,
                    out_return: has_out_return,
                });
            }
        }

        result
    }

    fn build_structs(namespace: &general::Namespace) -> Vec<PlayerStructDef> {
        let mut result = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::Struct(s) = def {
                let fields: Vec<PlayerFieldDef> = s
                    .fields
                    .iter()
                    .map(|f| PlayerFieldDef {
                        name: f.name.clone(),
                        type_expr: ffi_type_to_player(&f.ty.ty),
                    })
                    .collect();

                result.push(PlayerStructDef {
                    name: s.name.0.clone(),
                    fields,
                });
            }
        }

        result
    }

    /// Build a `TsFfiModule` for the player, using raw symbol names
    /// (no `ubrn_` prefix) so the interface matches what `register()` returns.
    fn build_typed_module(
        namespace: &general::Namespace,
        names_future_family: bool,
        installs_string_helpers: bool,
        async_entries: &HashMap<String, AsyncEntry>,
    ) -> TsFfiModule {
        use heck::ToUpperCamelCase;
        let module_name = format!("Native{}", namespace.name.to_upper_camel_case());

        // No bless_pointer for the player — the player runtime handles that
        // internally. The synthetic string helpers, however, ARE installed on
        // the native module by the player shim (createFromUtf8 fast path) for
        // flavors without a JS TextDecoder (Jsi2), so the converter's calls to
        // them must typecheck against NativeModuleInterface. Their names keep
        // the `ubrn_` prefix to match build_string_helper / the shim methods.
        let mut functions = if installs_string_helpers {
            TsFfiModule::synthetic_string_functions()
        } else {
            Vec::new()
        };

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::RustFunction(func) = def {
                if TsFfiModule::should_include_function(func, names_future_family) {
                    // Use raw symbol name (no ubrn_ prefix)
                    functions.push(Self::map_ffi_function_for_player(
                        func,
                        async_entries.get(&func.name.0),
                    ));
                }
            }
        }

        let definitions = TsFfiModule::build_definitions(namespace);

        TsFfiModule {
            module_name,
            strict_type_checking: false,
            is_jsi: false,
            functions,
            definitions,
            has_continuation_callback: false,
            has_foreign_future: false,
        }
    }

    fn map_ffi_function_for_player(
        func: &general::FfiFunction,
        async_entry: Option<&AsyncEntry>,
    ) -> FfiFunctionDecl {
        let mut arguments: Vec<FfiArgDecl> = func
            .arguments
            .iter()
            .map(|arg| FfiArgDecl {
                name: arg.name.to_lower_camel_case(),
                type_name: ffi_type_to_ts(&arg.ty.ty),
            })
            .collect();

        // The player's entry for an async function takes the status the
        // future completes into, and a signal that cancels it, then resolves
        // with the completed value. Both names keep the `uniffi_` prefix:
        // user arguments are camelCased, so they cannot collide.
        if let Some(entry) = async_entry {
            arguments.push(FfiArgDecl {
                name: "uniffi_out_err".into(),
                type_name: "UniffiRustCallStatus".into(),
            });
            arguments.push(FfiArgDecl {
                name: "uniffi_signal".into(),
                type_name: "AbortSignal | undefined".into(),
            });
            return FfiFunctionDecl {
                name: func.name.0.clone(),
                arguments,
                return_type: entry.ts_ret.clone(),
                returns_promise: true,
            };
        }

        if func.has_rust_call_status_arg {
            arguments.push(FfiArgDecl {
                name: "uniffi_out_err".into(),
                type_name: "UniffiRustCallStatus".into(),
            });
        }

        let return_type = func
            .return_type
            .ty
            .as_ref()
            .map(|rt| ffi_type_to_ts(&rt.ty));

        FfiFunctionDecl {
            name: func.name.0.clone(),
            arguments,
            return_type,
            returns_promise: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_void_pointer_out_return_emits_void() {
        assert_eq!(
            ret_tag_for_out_return(&general::FfiType::VoidPointer),
            "FfiType.Void"
        );
    }

    #[test]
    fn a_reference_out_return_still_unwraps_to_its_inner_type() {
        let inner = general::FfiType::Reference(Box::new(general::FfiType::UInt32));
        assert_eq!(ret_tag_for_out_return(&inner), "FfiType.UInt32");
    }

    #[test]
    fn a_plain_out_return_type_is_emitted_as_itself() {
        assert_eq!(
            ret_tag_for_out_return(&general::FfiType::UInt64),
            "FfiType.UInt64"
        );
    }

    /// The wasm2 fixtures run under tsx, which strips types unchecked, so only
    /// this catches a user argument colliding with the entry's own parameters.
    #[test]
    fn an_async_entry_takes_a_status_and_a_signal_beside_a_user_argument_named_signal() {
        let handle = general::FfiType::Handle(general::HandleKind::RustFuture);
        let export =
            |verb: &str| general::RustFfiFunctionName(format!("ffi_x_rust_future_{verb}_u8"));
        let start = general::FfiFunction {
            name: general::RustFfiFunctionName("uniffi_x_fn_func_echo_signal".into()),
            async_data: Some(general::AsyncData {
                ffi_rust_future_poll: export("poll"),
                ffi_rust_future_cancel: export("cancel"),
                ffi_rust_future_free: export("free"),
                ffi_rust_future_complete: export("complete"),
                ffi_foreign_future_complete: general::FfiFunctionTypeName("unused".into()),
                ffi_foreign_future_result: general::FfiStructName("unused".into()),
            }),
            arguments: vec![general::FfiArgument::new("signal", general::FfiType::UInt8)],
            return_type: general::FfiReturnType {
                ty: Some(handle.clone().into()),
            },
            kind: general::FfiFunctionKind::Scaffolding,
            ..Default::default()
        };
        let complete = general::FfiFunction {
            name: export("complete"),
            arguments: vec![general::FfiArgument::new("handle", handle)],
            return_type: general::FfiReturnType {
                ty: Some(general::FfiType::UInt8.into()),
            },
            has_rust_call_status_arg: true,
            kind: general::FfiFunctionKind::RustFutureComplete,
            ..Default::default()
        };
        let entries = async_entries(&[start.clone().into(), complete.into()]);

        let decl = PlayerFfiModule::map_ffi_function_for_player(&start, entries.get(&start.name.0));
        let args: Vec<_> = decl
            .arguments
            .iter()
            .map(|a| (a.name.as_str(), a.type_name.as_str()))
            .collect();
        assert_eq!(
            args,
            [
                ("signal", "number"),
                ("uniffi_out_err", "UniffiRustCallStatus"),
                ("uniffi_signal", "AbortSignal | undefined")
            ]
        );
        assert_eq!(decl.return_type.as_deref(), Some("number"));
        assert!(decl.returns_promise);
    }
}
