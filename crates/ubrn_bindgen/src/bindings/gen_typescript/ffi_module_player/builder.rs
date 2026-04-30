/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use heck::ToLowerCamelCase;
use uniffi_bindgen::pipeline::general;

use super::nodes::*;
use super::type_mapping::ffi_type_to_player;
use crate::bindings::gen_typescript::config::TsConfig;
use crate::bindings::gen_typescript::ffi_module::type_mapping::ffi_type_to_ts;
use crate::bindings::gen_typescript::ffi_module::{
    namespace_has_async, FfiArgDecl, FfiFunctionDecl, TsFfiModule,
};

impl PlayerFfiModule {
    pub(crate) fn from_general(
        namespace: &general::Namespace,
        config: &TsConfig,
        lib_path: Option<String>,
    ) -> Self {
        let has_async = namespace_has_async(namespace);

        let symbols = Self::build_symbols(namespace);
        let functions = Self::build_functions(namespace, has_async);
        let callbacks = Self::build_callbacks(namespace);
        let structs = Self::build_structs(namespace);

        // Reuse TsFfiModule for the typed interface, but we'll use raw
        // symbol names (no ubrn_ prefix) in the NativeModuleInterface.
        let ts_module = Self::build_typed_module(namespace, has_async);

        Self {
            strict_type_checking: config.strict_type_checking,
            lib_path,
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

    fn build_functions(namespace: &general::Namespace, has_async: bool) -> Vec<PlayerFunctionDef> {
        let mut result = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::RustFunction(func) = def {
                if TsFfiModule::should_include_function(func, has_async) {
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
                    });
                }
            }
        }

        result
    }

    fn build_callbacks(namespace: &general::Namespace) -> Vec<PlayerCallbackDef> {
        let mut result = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::FunctionType(ft) = def {
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
                        .map(|a| {
                            let inner = match &a.ty.ty {
                                general::FfiType::Reference(t)
                                | general::FfiType::MutReference(t) => t.as_ref(),
                                t => t,
                            };
                            ffi_type_to_player(inner)
                        })
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
    fn build_typed_module(namespace: &general::Namespace, has_async: bool) -> TsFfiModule {
        use heck::ToUpperCamelCase;
        let module_name = format!("Native{}", namespace.name.to_upper_camel_case());

        // No synthetic string functions or bless_pointer for the player —
        // the player runtime handles these internally.
        let mut functions = Vec::new();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::RustFunction(func) = def {
                if TsFfiModule::should_include_function(func, has_async) {
                    // Use raw symbol name (no ubrn_ prefix)
                    functions.push(Self::map_ffi_function_for_player(func));
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

    fn map_ffi_function_for_player(func: &general::FfiFunction) -> FfiFunctionDecl {
        let mut arguments: Vec<FfiArgDecl> = func
            .arguments
            .iter()
            .map(|arg| FfiArgDecl {
                name: arg.name.to_lower_camel_case(),
                type_name: ffi_type_to_ts(&arg.ty.ty),
            })
            .collect();

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
        }
    }
}
