/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Construct IR nodes from `general` pipeline types.

use std::collections::HashMap;

use heck::{ToLowerCamelCase, ToUpperCamelCase};
use uniffi_bindgen::pipeline::general;

use crate::bindings::gen_typescript::Config;
use crate::switches::AbiFlavor;

use super::docstring::{format_docstring, format_docstring_indented};
use super::nodes::*;
use super::type_helpers::*;

pub(super) fn build_optional(config: &Config, opt: &general::OptionalType) -> TsSimpleWrapper {
    TsSimpleWrapper {
        infra_class: "FfiConverterOptional".into(),
        ffi_converter_name: ffi_converter_name_for(config, &opt.self_type),
        type_label: type_label_for(config, &opt.self_type.ty),
        inner_converters: vec![ffi_converter_name_for(config, &opt.inner)],
    }
}

pub(super) fn build_sequence(config: &Config, seq: &general::SequenceType) -> TsSimpleWrapper {
    TsSimpleWrapper {
        infra_class: "FfiConverterArray".into(),
        ffi_converter_name: ffi_converter_name_for(config, &seq.self_type),
        type_label: type_label_for(config, &seq.self_type.ty),
        inner_converters: vec![ffi_converter_name_for(config, &seq.inner)],
    }
}

pub(super) fn build_map(config: &Config, map: &general::MapType) -> TsSimpleWrapper {
    TsSimpleWrapper {
        infra_class: "FfiConverterMap".into(),
        ffi_converter_name: ffi_converter_name_for(config, &map.self_type),
        type_label: type_label_for(config, &map.self_type.ty),
        inner_converters: vec![
            ffi_converter_name_for(config, &map.key),
            ffi_converter_name_for(config, &map.value),
        ],
    }
}

/// Apply the `ubrn_` prefix to a raw FFI symbol name when the flavor requires it.
fn ffi_name(flavor: &AbiFlavor, raw_name: &str) -> String {
    if flavor.supports_ubrn_prefix() {
        format!("ubrn_{}", raw_name)
    } else {
        raw_name.to_string()
    }
}

/// FFI function names match the `ffi_module` synthetic declarations.
pub(super) fn build_string_helper(flavor: &AbiFlavor) -> TsStringHelper {
    // Most backends have a global TextEncoder/TextDecoder. But, some do not e.g. JSI.
    // In these cases we provide C++ (or Rust) implementations for the string conversion
    // functions instead.
    let supports_text_encoder = flavor.supports_text_encoder();
    TsStringHelper {
        supports_text_encoder,
        ffi_string_to_arraybuffer: "ubrn_uniffi_internal_fn_func_ffi__string_to_arraybuffer".into(),
        ffi_arraybuffer_to_string: "ubrn_uniffi_internal_fn_func_ffi__arraybuffer_to_string".into(),
        ffi_string_to_bytelength: "ubrn_uniffi_internal_fn_func_ffi__string_to_byte_length".into(),
    }
}

/// Custom lift/lower instructions are stored within the configuration table within the `custom_types` section,
/// keyed by the custom type's name.
///
/// If no such instructions are present in the configuration, falls back to the builtin converter.
pub(super) fn build_custom_type(config: &Config, custom: &general::CustomType) -> TsCustomType {
    let type_name = custom.name.clone();
    let ffi_converter_name = ffi_converter_name_for(config, &custom.self_type);
    let builtin_type_name = type_label_for(config, &custom.builtin.ty);
    let builtin_ffi_converter = ffi_converter_name_for(config, &custom.builtin);
    let ffi_type_name = ffi_type_to_ts_name(&custom.builtin.ffi_type.ty);

    let custom_config = config
        .custom_types
        .get(&custom.name)
        .map(|cfg| TsCustomConfig {
            concrete_type_name: cfg.type_name.clone(),
            imports: cfg.imports.clone(),
            lift_expr: cfg.lift("intermediate"),
            lower_expr: cfg.lower("value"),
        });

    TsCustomType {
        type_name,
        ffi_converter_name,
        builtin_type_name,
        builtin_ffi_converter,
        ffi_type_name,
        custom_config,
    }
}

pub(super) fn build_external_type(
    config: &Config,
    external: &general::ExternalType,
) -> TsExternalType {
    let module_path = external.namespace.clone();
    let type_name = type_label_for(config, &external.self_type.ty);
    let converter_name = ffi_converter_name_for(config, &external.self_type);
    let is_enum_type = matches!(external.self_type.ty, general::Type::Enum { .. });

    TsExternalType {
        module_path,
        type_name,
        converter_name,
        is_enum_type,
    }
}

fn render_literal(config: &Config, lit: &general::Literal) -> String {
    match lit {
        general::Literal::Boolean(b) => b.to_string(),
        general::Literal::String(s) => format!("\"{}\"", s),
        general::Literal::Int(n, _, type_node) => match &type_node.ty {
            general::Type::Int64 | general::Type::UInt64 => format!("BigInt(\"{}\")", n),
            _ => n.to_string(),
        },
        general::Literal::UInt(n, _, type_node) => match &type_node.ty {
            general::Type::Int64 | general::Type::UInt64 => format!("BigInt(\"{}\")", n),
            _ => n.to_string(),
        },
        general::Literal::Float(s, _) => s.clone(),
        general::Literal::Enum(variant, type_node) => {
            // No containing enum context here, so just use the UpperCamelCase variant name.
            let type_name = type_label_for(config, &type_node.ty);
            let variant_name = variant.to_upper_camel_case();
            format!("{type_name}.{variant_name}")
        }
        general::Literal::EmptySequence => "[]".into(),
        general::Literal::EmptyMap => "new Map()".into(),
        general::Literal::None => "undefined".into(),
        general::Literal::Some { inner } => render_default_value(config, inner),
    }
}

fn render_default_value(config: &Config, dv: &general::DefaultValue) -> String {
    match dv {
        general::DefaultValue::Literal(lit_node) => render_literal(config, &lit_node.lit),
        general::DefaultValue::Default(_) => "undefined".into(),
    }
}

/// Discriminants stay in JS (not sent across the FFI), so large integers
/// can be safely narrowed to JS numbers when they fit.
fn render_variant_discr(config: &Config, discr: &general::LiteralNode) -> String {
    match &discr.lit {
        general::Literal::String(s) => format!("\"{}\"", s),
        general::Literal::UInt(n, _, type_node) => match &type_node.ty {
            general::Type::Int64 | general::Type::UInt64 => {
                if *n < u32::MAX as u64 {
                    format!("{}", n)
                } else {
                    format!("\"{}\"", n)
                }
            }
            _ => n.to_string(),
        },
        general::Literal::Int(n, _, type_node) => match &type_node.ty {
            general::Type::Int64 | general::Type::UInt64 => {
                if (i32::MIN as i64) < *n && *n < (i32::MAX as i64) {
                    format!("{}", n)
                } else {
                    format!("\"{}\"", n)
                }
            }
            _ => n.to_string(),
        },
        other => format!("\"{}\"", render_literal(config, other)),
    }
}

pub(super) fn build_field(config: &Config, field: &general::Field) -> TsField {
    let name = field.name.to_lower_camel_case();
    let is_optional = matches!(&field.ty.ty, general::Type::Optional { .. });
    let ts_type = if is_optional {
        // Unwrap Optional<T> to just T; the `?:` in the type declaration handles optionality.
        match &field.ty.ty {
            general::Type::Optional { inner_type } => type_label_for(config, inner_type),
            _ => unreachable!(),
        }
    } else {
        type_label_for(config, &field.ty.ty)
    };
    let ffi_converter = ffi_converter_name_for(config, &field.ty);
    let default_value = field
        .default
        .as_ref()
        .map(|default| render_default_value(config, default));
    let docstring = field.docstring.as_deref().map(format_docstring_indented);
    TsField {
        name,
        ts_type,
        is_optional,
        ffi_converter,
        default_value,
        docstring,
    }
}

pub(super) fn build_variant(config: &Config, variant: &general::Variant) -> TsVariant {
    let name = variant.name.to_upper_camel_case();
    let docstring = variant.docstring.as_deref().map(format_docstring_indented);
    let discriminant = render_variant_discr(config, &variant.discr);
    let fields: Vec<TsField> = variant
        .fields
        .iter()
        .map(|field| build_field(config, field))
        .collect();
    let has_nameless_fields = matches!(variant.fields_kind, general::FieldsKind::Unnamed);
    TsVariant {
        name,
        docstring,
        discriminant,
        fields,
        has_nameless_fields,
    }
}

/// `Some` only when Rust declares an explicit discriminant type (e.g. `#[repr(u8)]`).
fn discr_type_for(config: &Config, en: &general::Enum) -> Option<String> {
    en.meta_discr_type
        .as_ref()
        .map(|t| type_label_for(config, &t.ty))
}

pub(super) fn build_enum(config: &Config, en: &general::Enum, flavor: &AbiFlavor) -> TsEnum {
    let ts_name = rewrite_js_builtins(&en.name.to_upper_camel_case());
    let ffi_converter_name = ffi_converter_name_for(config, &en.self_type);
    let docstring = en.docstring.as_deref().map(format_docstring);

    let is_error = matches!(en.shape, general::EnumShape::Error { .. });
    let is_flat = en.is_flat;

    let discr_type = discr_type_for(config, en);

    let variants: Vec<TsVariant> = en
        .variants
        .iter()
        .map(|variant| build_variant(config, variant))
        .collect();

    let uniffi_traits = build_uniffi_traits_value(
        config,
        &en.uniffi_trait_methods,
        &ffi_converter_name,
        flavor,
    );
    let constructors = en
        .constructors
        .iter()
        .map(|c| build_constructor_callable(config, c, flavor))
        .collect();
    let methods = en
        .methods
        .iter()
        .map(|m| build_value_method_callable(config, m, &ffi_converter_name, flavor))
        .collect();

    TsEnum {
        ts_name,
        ffi_converter_name,
        docstring,
        is_flat,
        is_error,
        discr_type,
        variants,
        uniffi_traits,
        constructors,
        methods,
    }
}

pub(super) fn build_record(config: &Config, rec: &general::Record, flavor: &AbiFlavor) -> TsRecord {
    let ts_name = rewrite_js_builtins(&rec.name.to_upper_camel_case());
    let ffi_converter_name = ffi_converter_name_for(config, &rec.self_type);
    let docstring = rec.docstring.as_deref().map(format_docstring);

    let fields: Vec<TsField> = rec
        .fields
        .iter()
        .map(|field| build_field(config, field))
        .collect();

    // Suppress default TS factory helpers when Rust defines a constructor with the same name.
    let has_create_constructor = rec.constructors.iter().any(|c| c.callable.name == "create");
    let has_new_constructor = rec.constructors.iter().any(|c| c.callable.name == "new");

    let uniffi_traits = build_uniffi_traits_value(
        config,
        &rec.uniffi_trait_methods,
        &ffi_converter_name,
        flavor,
    );
    let constructors = rec
        .constructors
        .iter()
        .map(|c| build_constructor_callable(config, c, flavor))
        .collect();
    let methods = rec
        .methods
        .iter()
        .map(|m| build_value_method_callable(config, m, &ffi_converter_name, flavor))
        .collect();

    TsRecord {
        ts_name,
        ffi_converter_name,
        docstring,
        fields,
        has_create_constructor,
        has_new_constructor,
        uniffi_traits,
        constructors,
        methods,
    }
}

/// Objects get `FfiConverter{name}__as_error`; other types use the regular converter name.
fn ffi_error_converter_for(config: &Config, type_node: &general::TypeNode) -> String {
    let mut name = ffi_converter_name_for(config, type_node);
    if matches!(&type_node.ty, general::Type::Interface { .. }) {
        name.push_str("__as_error");
    }
    name
}

fn build_error_type(config: &Config, type_node: &general::TypeNode) -> TsErrorType {
    let ffi_error_converter = ffi_error_converter_for(config, type_node);
    let lift_error_fn = format!("{ffi_error_converter}.lift.bind({ffi_error_converter})");
    let lower_error_fn = format!("{ffi_error_converter}.lower.bind({ffi_error_converter})");
    // Enums use type_label; objects use the class name directly.
    let decl_type_name = match &type_node.ty {
        general::Type::Interface { name, .. } => name.to_upper_camel_case(),
        _ => type_label_for(config, &type_node.ty),
    };
    TsErrorType {
        lift_error_fn,
        lower_error_fn,
        decl_type_name,
    }
}

pub(super) fn build_arg(config: &Config, arg: &general::Argument) -> TsArg {
    TsArg {
        name: arg_name(&arg.name),
        ts_type: type_label_for(config, &arg.ty.ty),
        ffi_converter: ffi_converter_name_for(config, &arg.ty),
        default_value: arg
            .default
            .as_ref()
            .map(|default| render_default_value(config, default)),
    }
}

/// `receiver`: `None` for top-level functions and constructors,
/// `Some(Pointer)` for object methods.
pub(super) fn build_callable(
    config: &Config,
    callable: &general::Callable,
    docstring: &Option<String>,
    receiver: Option<TsReceiver>,
    flavor: &AbiFlavor,
) -> TsCallable {
    let name = fn_name(&callable.name);
    let arguments: Vec<TsArg> = callable
        .arguments
        .iter()
        .map(|arg| build_arg(config, arg))
        .collect();
    let return_type = callable.return_type.ty.as_ref().map(|tn| TsReturnType {
        ts_type: type_label_for(config, &tn.ty),
        ffi_converter: ffi_converter_name_for(config, tn),
        ffi_type: ffi_type_to_ts_name(&tn.ffi_type.ty),
    });
    let throws = callable
        .throws_type
        .ty
        .as_ref()
        .map(|type_node| build_error_type(config, type_node));
    let callable_ffi_name = ffi_name(flavor, &callable.ffi_func.0);
    let ffi_async = callable.async_data.as_ref().map(|ad| TsAsyncFfi {
        poll: ffi_name(flavor, &ad.ffi_rust_future_poll.0),
        complete: ffi_name(flavor, &ad.ffi_rust_future_complete.0),
        free: ffi_name(flavor, &ad.ffi_rust_future_free.0),
        cancel: ffi_name(flavor, &ad.ffi_rust_future_cancel.0),
    });

    TsCallable {
        name,
        docstring: docstring.as_deref().map(format_docstring),
        arguments,
        return_type,
        throws,
        ffi_name: callable_ffi_name,
        ffi_async,
        receiver,
    }
}

pub(super) fn build_method_callable(
    config: &Config,
    method: &general::Method,
    _ffi_clone_name: &str,
    flavor: &AbiFlavor,
) -> TsCallable {
    let receiver = Some(TsReceiver::Pointer);
    build_callable(
        config,
        &method.callable,
        &method.docstring,
        receiver,
        flavor,
    )
}

pub(super) fn build_constructor_callable(
    config: &Config,
    cons: &general::Constructor,
    flavor: &AbiFlavor,
) -> TsCallable {
    build_callable(config, &cons.callable, &cons.docstring, None, flavor)
}

fn collect_uniffi_traits(
    tm: &general::UniffiTraitMethods,
    build_method: impl Fn(&general::Method) -> TsCallable,
) -> Vec<TsUniffiTrait> {
    let mut traits = Vec::new();

    if let Some(ref fmt) = tm.display_fmt {
        traits.push(TsUniffiTrait::Display {
            method: build_method(fmt),
        });
    }
    if let Some(ref fmt) = tm.debug_fmt {
        traits.push(TsUniffiTrait::Debug {
            method: build_method(fmt),
        });
    }
    if let (Some(ref eq), Some(ref ne)) = (&tm.eq_eq, &tm.eq_ne) {
        traits.push(TsUniffiTrait::Eq {
            eq: build_method(eq),
            ne: Box::new(build_method(ne)),
        });
    }
    if let Some(ref hash) = tm.hash_hash {
        traits.push(TsUniffiTrait::Hash {
            method: build_method(hash),
        });
    }
    if let Some(ref cmp) = tm.ord_cmp {
        traits.push(TsUniffiTrait::Ord {
            cmp: build_method(cmp),
        });
    }

    traits
}

pub(super) fn build_uniffi_traits(
    config: &Config,
    tm: &general::UniffiTraitMethods,
    ffi_clone_name: &str,
    flavor: &AbiFlavor,
) -> Vec<TsUniffiTrait> {
    collect_uniffi_traits(tm, |m| {
        build_method_callable(config, m, ffi_clone_name, flavor)
    })
}

pub(super) fn build_value_method_callable(
    config: &Config,
    method: &general::Method,
    ffi_converter: &str,
    flavor: &AbiFlavor,
) -> TsCallable {
    let receiver = Some(TsReceiver::Value {
        ffi_converter: ffi_converter.to_string(),
    });
    build_callable(
        config,
        &method.callable,
        &method.docstring,
        receiver,
        flavor,
    )
}

pub(super) fn build_uniffi_traits_value(
    config: &Config,
    tm: &general::UniffiTraitMethods,
    ffi_converter: &str,
    flavor: &AbiFlavor,
) -> Vec<TsUniffiTrait> {
    collect_uniffi_traits(tm, |m| {
        build_value_method_callable(config, m, ffi_converter, flavor)
    })
}

pub(super) fn build_object(
    config: &Config,
    interface: &general::Interface,
    flavor: &AbiFlavor,
    ffi_fn_types: &HashMap<String, &general::FfiFunctionType>,
    strict_object_types: bool,
) -> TsObject {
    let class_name = rewrite_js_builtins(&interface.name.to_upper_camel_case());
    let is_trait = !interface.imp.has_struct();
    let is_error = interface.self_type.is_used_as_error;
    let has_callback_interface = interface.imp.has_callback_interface();

    // Non-trait/error: protocol = "{Name}Like", impl = "{Name}"
    // Trait (non-error): protocol = "{Name}",  impl = "{Name}Impl"
    let (protocol_name, impl_class_name) = if !is_trait || is_error {
        (format!("{class_name}Like"), class_name.clone())
    } else {
        (class_name.clone(), format!("{class_name}Impl"))
    };

    let ts_name = class_name.clone();
    let decl_type_name = impl_class_name.clone();
    let obj_factory = format!("uniffiType{impl_class_name}ObjectFactory");
    let ffi_converter_name = ffi_converter_name_for(config, &interface.self_type);
    let ffi_error_converter_name = format!("{ffi_converter_name}__as_error");

    let docstring = interface.docstring.as_deref().map(format_docstring);

    let ffi_clone = ffi_name(flavor, &interface.ffi_func_clone.0);
    let ffi_free = ffi_name(flavor, &interface.ffi_func_free.0);
    let ffi_bless_pointer = ffi_name(
        flavor,
        &format!(
            "uniffi_internal_fn_method_{}_ffi__bless_pointer",
            interface.name.to_ascii_lowercase()
        ),
    );

    let (primary_constructor, alternate_constructors) = {
        let mut primary = None;
        let mut alternates = Vec::new();
        for cons in &interface.constructors {
            let built = build_constructor_callable(config, cons, flavor);
            if matches!(
                cons.callable.kind,
                general::CallableKind::Constructor { primary: true, .. }
            ) {
                primary = Some(built);
            } else {
                alternates.push(built);
            }
        }
        (primary, alternates)
    };

    let methods: Vec<TsMethod> = interface
        .methods
        .iter()
        .map(|m| build_method_callable(config, m, &ffi_clone, flavor))
        .collect();

    let uniffi_traits =
        build_uniffi_traits(config, &interface.uniffi_trait_methods, &ffi_clone, flavor);

    let supports_finalization_registry = flavor.supports_finalization_registry();

    let vtable = interface
        .vtable
        .as_ref()
        .map(|vt| build_vtable(config, vt, ffi_fn_types, flavor));

    let trait_impl = format!("uniffiCallbackInterface{}", class_name);

    TsObject {
        ts_name,
        decl_type_name,
        impl_class_name,
        protocol_name,
        obj_factory,
        ffi_converter_name,
        ffi_error_converter_name,
        docstring,
        is_error,
        vtable,
        trait_impl,
        primary_constructor,
        alternate_constructors,
        methods,
        uniffi_traits,
        ffi_bless_pointer,
        ffi_clone,
        ffi_free,
        supports_finalization_registry,
        has_callback_interface,
        strict_object_types,
    }
}

pub(super) fn build_vtable(
    config: &Config,
    vt: &general::VTable,
    ffi_fn_types: &HashMap<String, &general::FfiFunctionType>,
    flavor: &AbiFlavor,
) -> TsVtable {
    let ffi_init_fn = ffi_name(flavor, &vt.init_fn.0);
    let fields = vt
        .methods
        .iter()
        .map(|vm| build_vtable_field(config, vm, ffi_fn_types, flavor))
        .collect();
    TsVtable {
        ffi_init_fn,
        fields,
    }
}

fn build_vtable_field(
    config: &Config,
    vm: &general::VTableMethod,
    ffi_fn_types: &HashMap<String, &general::FfiFunctionType>,
    flavor: &AbiFlavor,
) -> TsVtableField {
    let name = vm.callable.name.clone();
    let method = Some(build_callable(config, &vm.callable, &None, None, flavor));

    let general::FfiType::Function(ref ffi_fn_name) = vm.ffi_type.ty else {
        return TsVtableField {
            name,
            method,
            foreign_future_result: None,
            ffi_closure_args: Vec::new(),
            has_rust_call_status_arg: false,
        };
    };

    let foreign_future_result = build_foreign_future_result(&vm.callable);
    let (ffi_closure_args, has_rust_call_status_arg) =
        build_closure_args(ffi_fn_name, ffi_fn_types, &vm.callable);

    TsVtableField {
        name,
        method,
        foreign_future_result,
        ffi_closure_args,
        has_rust_call_status_arg,
    }
}

fn build_foreign_future_result(callable: &general::Callable) -> Option<TsForeignFutureResult> {
    callable
        .async_data
        .as_ref()
        .map(|ad| TsForeignFutureResult {
            struct_name: format!(
                "Uniffi{}",
                ad.ffi_foreign_future_result.0.to_upper_camel_case()
            ),
            return_ffi_default_value: callable
                .return_type
                .ty
                .as_ref()
                .map(|tn| ffi_default_value_for(&tn.ffi_type.ty))
                .unwrap_or_default(),
        })
}

/// Resolve the closure argument list from the FfiFunctionType lookup table,
/// falling back to reconstructing from the callable's own arguments.
fn build_closure_args(
    ffi_fn_name: &general::FfiFunctionTypeName,
    ffi_fn_types: &HashMap<String, &general::FfiFunctionType>,
    callable: &general::Callable,
) -> (Vec<TsFfiArg>, bool) {
    if let Some(ffi_fn) = ffi_fn_types.get(&ffi_fn_name.0) {
        let args = ffi_fn
            .arguments
            .iter()
            .filter(|a| a.name != "uniffi_out_return" && a.name != "uniffi_out_dropped_callback")
            .map(|a| TsFfiArg {
                name: arg_name(&a.name),
                ffi_type: ffi_type_to_ts_name(&a.ty.ty),
            })
            .collect();
        (args, ffi_fn.has_rust_call_status_arg)
    } else {
        let mut args = vec![TsFfiArg {
            name: "uniffiHandle".into(),
            ffi_type: "bigint".into(),
        }];
        args.extend(callable.arguments.iter().map(|a| TsFfiArg {
            name: arg_name(&a.name),
            ffi_type: ffi_type_to_ts_name(&a.ty.ffi_type.ty),
        }));
        (args, false)
    }
}

pub(super) fn build_callback_interface(
    config: &Config,
    cbi: &general::CallbackInterface,
    ffi_fn_types: &HashMap<String, &general::FfiFunctionType>,
    flavor: &AbiFlavor,
) -> TsCallbackInterface {
    let ts_name = rewrite_js_builtins(&cbi.name.to_upper_camel_case());
    // Callback interfaces use `FfiConverterType{Name}` (not `FfiConverter{canonical_name}`).
    let ffi_converter_name = format!("FfiConverterType{ts_name}");

    let docstring = cbi.docstring.as_deref().map(format_docstring);

    // No receiver: Rust calls these methods, not JS.
    let methods: Vec<TsCallable> = cbi
        .methods
        .iter()
        .map(|m| build_callable(config, &m.callable, &m.docstring, None, flavor))
        .collect();

    let has_async_methods = cbi.methods.iter().any(|m| m.callable.is_async());

    let vtable = build_vtable(config, &cbi.vtable, ffi_fn_types, flavor);

    let trait_impl = format!("uniffiCallbackInterface{ts_name}");

    TsCallbackInterface {
        protocol_name: ts_name.clone(),
        ts_name,
        ffi_converter_name,
        trait_impl,
        docstring,
        methods,
        vtable,
        has_async_methods,
    }
}

pub(super) fn build_functions(
    config: &Config,
    namespace: &general::Namespace,
    flavor: &AbiFlavor,
) -> Vec<TsFunction> {
    namespace
        .functions
        .iter()
        .map(|f| build_callable(config, &f.callable, &f.docstring, None, flavor))
        .collect()
}

pub(super) fn build_initialization(
    namespace: &general::Namespace,
    flavor: &AbiFlavor,
) -> InitializationIR {
    let bindings_contract_version = namespace.correct_contract_version.clone();
    let ffi_contract_version_fn = ffi_name(flavor, &namespace.ffi_uniffi_contract_version.0);

    let checksums: Vec<TsChecksum> = namespace
        .checksums
        .iter()
        .map(|c| TsChecksum {
            raw_name: c.fn_name.0.clone(),
            ffi_fn_name: ffi_name(flavor, &c.fn_name.0),
            expected_value: c.checksum.to_string(),
        })
        .collect();

    // Only callback interfaces and trait interfaces with callbacks need
    // initialization functions (vtable registration).
    let mut initialization_fns = Vec::new();
    for td in &namespace.type_definitions {
        match td {
            general::TypeDefinition::Interface(i) => {
                if i.imp.has_callback_interface() {
                    initialization_fns.push(format!(
                        "uniffiCallbackInterface{}.register",
                        i.name.to_upper_camel_case()
                    ));
                }
            }
            general::TypeDefinition::CallbackInterface(ci) => {
                initialization_fns.push(format!(
                    "uniffiCallbackInterface{}.register",
                    ci.name.to_upper_camel_case()
                ));
            }
            _ => {}
        }
    }

    InitializationIR {
        bindings_contract_version,
        ffi_contract_version_fn,
        checksums,
        initialization_fns,
    }
}
