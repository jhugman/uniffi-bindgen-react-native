/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use heck::{ToLowerCamelCase, ToUpperCamelCase};
use uniffi_bindgen::pipeline::general;

use super::nodes::*;
use super::type_mapping::{ffi_type_to_ts, ffi_type_to_ts_native};
use crate::switches::AbiFlavor;

impl TsFfiModule {
    pub(crate) fn from_general(
        namespace: &general::Namespace,
        flavor: &AbiFlavor,
        config: &super::super::Config,
    ) -> Self {
        let is_jsi = matches!(flavor, AbiFlavor::Jsi);
        let module_name = format!("Native{}", namespace.name.to_upper_camel_case());

        let has_async = namespace.ffi_definitions.iter().any(|def| {
            matches!(
                def,
                general::FfiDefinition::RustFunction(f)
                    if matches!(
                        f.kind,
                        general::FfiFunctionKind::RustFuturePoll
                            | general::FfiFunctionKind::RustFutureComplete
                    )
            )
        });

        let functions = Self::build_functions(namespace, has_async);
        let definitions = Self::build_definitions(namespace);

        let has_continuation_callback = definitions.iter().any(|d| {
            matches!(d,
                FfiDefinitionDecl::Callback(cb) if cb.name == "UniffiRustFutureContinuationCallback"
            )
        });
        let has_foreign_future = definitions.iter().any(|d| {
            matches!(d,
                FfiDefinitionDecl::Struct(s) if s.name == "UniffiForeignFuture"
            )
        });

        Self {
            module_name,
            strict_type_checking: config.strict_type_checking,
            is_jsi,
            functions,
            definitions,
            has_continuation_callback,
            has_foreign_future,
        }
    }

    fn build_functions(namespace: &general::Namespace, has_async: bool) -> Vec<FfiFunctionDecl> {
        let mut functions = Self::synthetic_string_functions();

        for def in &namespace.ffi_definitions {
            if let general::FfiDefinition::RustFunction(func) = def {
                if Self::should_include_function(func, has_async) {
                    functions.push(Self::map_ffi_function(func, false));
                }
            }
        }

        for type_def in &namespace.type_definitions {
            if let general::TypeDefinition::Interface(interface) = type_def {
                functions.push(Self::synthetic_bless_pointer(&interface.name));
            }
        }

        functions
    }

    pub(crate) fn should_include_function(func: &general::FfiFunction, has_async: bool) -> bool {
        match func.kind {
            general::FfiFunctionKind::RustBufferFromBytes
            | general::FfiFunctionKind::RustBufferFree
            | general::FfiFunctionKind::RustBufferAlloc
            | general::FfiFunctionKind::RustBufferReserve => false,

            general::FfiFunctionKind::RustFuturePoll
            | general::FfiFunctionKind::RustFutureComplete
            | general::FfiFunctionKind::RustFutureCancel
            | general::FfiFunctionKind::RustFutureFree => has_async,

            general::FfiFunctionKind::Scaffolding
            | general::FfiFunctionKind::ObjectClone
            | general::FfiFunctionKind::ObjectFree
            | general::FfiFunctionKind::RustVtableInit
            | general::FfiFunctionKind::UniffiContractVersion
            | general::FfiFunctionKind::Checksum => true,
        }
    }

    fn map_ffi_function(func: &general::FfiFunction, is_internal: bool) -> FfiFunctionDecl {
        let type_mapper: fn(&general::FfiType) -> String = if is_internal {
            ffi_type_to_ts_native
        } else {
            ffi_type_to_ts
        };

        let mut arguments: Vec<FfiArgDecl> = func
            .arguments
            .iter()
            .map(|arg| FfiArgDecl {
                name: arg.name.to_lower_camel_case(),
                type_name: type_mapper(&arg.ty.ty),
            })
            .collect();

        if func.has_rust_call_status_arg {
            arguments.push(FfiArgDecl {
                name: "uniffi_out_err".into(),
                type_name: "UniffiRustCallStatus".into(),
            });
        }

        let return_type = func.return_type.ty.as_ref().map(|rt| type_mapper(&rt.ty));

        FfiFunctionDecl {
            name: format!("ubrn_{}", func.name.0),
            arguments,
            return_type,
        }
    }

    fn synthetic_string_functions() -> Vec<FfiFunctionDecl> {
        vec![
            FfiFunctionDecl {
                name: "ubrn_uniffi_internal_fn_func_ffi__string_to_byte_length".into(),
                arguments: vec![
                    FfiArgDecl {
                        name: "string".into(),
                        type_name: "string".into(),
                    },
                    FfiArgDecl {
                        name: "uniffi_out_err".into(),
                        type_name: "UniffiRustCallStatus".into(),
                    },
                ],
                return_type: Some("number".into()),
            },
            FfiFunctionDecl {
                name: "ubrn_uniffi_internal_fn_func_ffi__string_to_arraybuffer".into(),
                arguments: vec![
                    FfiArgDecl {
                        name: "string".into(),
                        type_name: "string".into(),
                    },
                    FfiArgDecl {
                        name: "uniffi_out_err".into(),
                        type_name: "UniffiRustCallStatus".into(),
                    },
                ],
                return_type: Some("Uint8Array".into()),
            },
            FfiFunctionDecl {
                name: "ubrn_uniffi_internal_fn_func_ffi__arraybuffer_to_string".into(),
                arguments: vec![
                    FfiArgDecl {
                        name: "buffer".into(),
                        type_name: "Uint8Array".into(),
                    },
                    FfiArgDecl {
                        name: "uniffi_out_err".into(),
                        type_name: "UniffiRustCallStatus".into(),
                    },
                ],
                return_type: Some("string".into()),
            },
        ]
    }

    fn synthetic_bless_pointer(object_name: &str) -> FfiFunctionDecl {
        FfiFunctionDecl {
            name: format!(
                "ubrn_uniffi_internal_fn_method_{}_ffi__bless_pointer",
                object_name.to_ascii_lowercase()
            ),
            arguments: vec![
                FfiArgDecl {
                    name: "pointer".into(),
                    type_name: "bigint".into(),
                },
                FfiArgDecl {
                    name: "uniffi_out_err".into(),
                    type_name: "UniffiRustCallStatus".into(),
                },
            ],
            return_type: Some("UniffiGcObject".into()),
        }
    }

    pub(crate) fn build_definitions(namespace: &general::Namespace) -> Vec<FfiDefinitionDecl> {
        namespace
            .ffi_definitions
            .iter()
            .filter_map(|def| match def {
                general::FfiDefinition::FunctionType(ft) => {
                    Some(FfiDefinitionDecl::Callback(Self::map_callback(ft)))
                }
                general::FfiDefinition::Struct(s) => {
                    Some(FfiDefinitionDecl::Struct(Self::map_struct(s)))
                }
                general::FfiDefinition::RustFunction(_) => None,
            })
            .collect()
    }

    fn map_callback(ft: &general::FfiFunctionType) -> FfiCallbackDecl {
        let name_str = &ft.name.0;

        let is_user_callback = name_str.starts_with("CallbackInterface");
        let is_free = name_str == "CallbackInterfaceFree" || name_str == "ForeignFutureFree";
        let exported = !is_user_callback && !is_free;

        let arguments: Vec<FfiArgDecl> = ft
            .arguments
            .iter()
            .filter(|a| a.name != "uniffi_out_return" && a.name != "uniffi_out_dropped_callback")
            .map(|arg| FfiArgDecl {
                name: arg.name.to_lower_camel_case(),
                type_name: ffi_type_to_ts(&arg.ty.ty),
            })
            .collect();

        let has_return_out_param = ft
            .arguments
            .iter()
            .any(|a| a.name == "uniffi_out_return" || a.name == "uniffi_out_dropped_callback");
        let is_blocking =
            has_return_out_param || ft.has_rust_call_status_arg || ft.return_type.ty.is_some();

        let return_type = if is_blocking {
            let out_return_type = ft
                .arguments
                .iter()
                .find(|a| a.name == "uniffi_out_return" || a.name == "uniffi_out_dropped_callback")
                .and_then(|a| {
                    if matches!(&a.ty.ty, general::FfiType::VoidPointer) {
                        return None;
                    }
                    let inner = match &a.ty.ty {
                        general::FfiType::Reference(t) | general::FfiType::MutReference(t) => t,
                        t => return Some(ffi_type_to_ts(t)),
                    };
                    if matches!(inner.as_ref(), general::FfiType::VoidPointer) {
                        None
                    } else {
                        Some(ffi_type_to_ts(inner))
                    }
                });
            match out_return_type {
                Some(ty) => ty,
                None => "UniffiResult<void>".into(),
            }
        } else {
            "void".into()
        };

        FfiCallbackDecl {
            exported,
            name: format!("Uniffi{}", name_str.to_upper_camel_case()),
            arguments,
            return_type,
        }
    }

    fn map_struct(s: &general::FfiStruct) -> FfiStructDecl {
        let name_str = &s.name.0;

        let is_foreign_future = name_str.starts_with("ForeignFuture");
        let is_vtable = s
            .fields
            .iter()
            .any(|f| matches!(f.ty.ty, general::FfiType::Function(_)));
        let exported = is_vtable || is_foreign_future;

        let fields = s
            .fields
            .iter()
            .map(|f| FfiFieldDecl {
                name: f.name.to_lower_camel_case(),
                type_name: ffi_type_to_ts(&f.ty.ty),
            })
            .collect();

        FfiStructDecl {
            exported,
            name: format!("Uniffi{}", name_str.to_upper_camel_case()),
            fields,
        }
    }
}
