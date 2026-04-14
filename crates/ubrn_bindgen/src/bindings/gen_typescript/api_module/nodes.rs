/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! IR node struct/enum definitions for the API wrapper module.

pub(crate) enum TsTypeDefinition {
    SimpleWrapper(TsSimpleWrapper),
    StringHelper(TsStringHelper),
    Custom(TsCustomType),
    External(TsExternalType),
    FlatEnum(TsEnum),
    FlatError(TsEnum),
    TaggedEnum(TsEnum),
    Record(TsRecord),
    Object(Box<TsObject>),
    CallbackInterface(TsCallbackInterface),
}

pub(crate) struct TsSimpleWrapper {
    pub infra_class: String,
    pub ffi_converter_name: String,
    pub type_label: String,
    pub inner_converters: Vec<String>,
}

pub(crate) struct TsStringHelper {
    pub supports_text_encoder: bool,
    pub ffi_string_to_buffer: String,
    pub ffi_string_from_buffer: String,
    pub ffi_string_to_bytelength: String,
    pub ffi_read_string_from_buffer: String,
}

pub(crate) struct TsCustomType {
    pub type_name: String,
    pub ffi_converter_name: String,
    pub builtin_type_name: String,
    pub builtin_ffi_converter: String,
    pub ffi_type_name: String,
    pub custom_config: Option<TsCustomConfig>,
}

pub(crate) struct TsCustomConfig {
    pub concrete_type_name: Option<String>,
    pub imports: Vec<(String, String)>,
    pub lift_expr: String,
    pub lower_expr: String,
}

pub(crate) struct TsExternalType {
    pub module_path: String,
    pub type_name: String,
    pub converter_name: String,
    pub is_enum_type: bool,
}

pub(crate) struct TsField {
    pub name: String,
    pub ts_type: String,
    pub is_optional: bool,
    pub ffi_converter: String,
    pub default_value: Option<String>,
    pub docstring: Option<String>,
}

pub(crate) struct TsVariant {
    pub name: String,
    pub docstring: Option<String>,
    pub discriminant: String,
    pub fields: Vec<TsField>,
    pub has_nameless_fields: bool,
}

pub(crate) struct TsEnum {
    pub ts_name: String,
    pub ffi_converter_name: String,
    pub docstring: Option<String>,
    pub is_flat: bool,
    pub is_error: bool,
    pub discr_type: Option<String>,
    pub variants: Vec<TsVariant>,
    pub uniffi_traits: Vec<TsUniffiTrait>,
    pub constructors: Vec<TsConstructor>,
    pub methods: Vec<TsMethod>,
}

impl TsEnum {
    pub fn has_callables(&self) -> bool {
        !self.uniffi_traits.is_empty() || !self.constructors.is_empty() || !self.methods.is_empty()
    }
    pub fn has_display_trait(&self) -> bool {
        self.uniffi_traits
            .iter()
            .any(|t| matches!(t, TsUniffiTrait::Display { .. }))
    }
}

pub(crate) struct TsRecord {
    pub ts_name: String,
    pub ffi_converter_name: String,
    pub docstring: Option<String>,
    pub fields: Vec<TsField>,
    pub has_create_constructor: bool,
    pub has_new_constructor: bool,
    pub uniffi_traits: Vec<TsUniffiTrait>,
    pub constructors: Vec<TsConstructor>,
    pub methods: Vec<TsMethod>,
}

impl TsRecord {
    pub fn has_callables(&self) -> bool {
        !self.uniffi_traits.is_empty() || !self.constructors.is_empty() || !self.methods.is_empty()
    }

    pub fn has_display_trait(&self) -> bool {
        self.uniffi_traits
            .iter()
            .any(|t| matches!(t, TsUniffiTrait::Display { .. }))
    }
}

// ---------------------------------------------------------------------------
// Callable IR nodes
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct TsCallable {
    pub name: String,
    pub docstring: Option<String>,
    pub arguments: Vec<TsArg>,
    pub return_type: Option<TsReturnType>,
    pub throws: Option<TsErrorType>,
    pub ffi_name: String,
    pub ffi_async: Option<TsAsyncFfi>,
    pub receiver: Option<TsReceiver>,
}

impl TsCallable {
    pub fn is_async(&self) -> bool {
        self.ffi_async.is_some()
    }
    pub fn is_throwing(&self) -> bool {
        self.throws.is_some()
    }
    /// The FfiConverter name for value receivers; `None` for pointer/no receiver.
    pub fn value_receiver_ffi_converter(&self) -> Option<&str> {
        match &self.receiver {
            Some(TsReceiver::Value { ffi_converter }) => Some(ffi_converter.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) enum TsReceiver {
    Pointer,
    Value { ffi_converter: String },
}

#[derive(Clone)]
pub(crate) struct TsArg {
    pub name: String,
    pub ts_type: String,
    pub ffi_converter: String,
    pub default_value: Option<String>,
}

#[derive(Clone)]
pub(crate) struct TsReturnType {
    pub ts_type: String,
    pub ffi_converter: String,
    pub ffi_type: String,
}

#[derive(Clone)]
pub(crate) struct TsErrorType {
    pub lift_error_fn: String,
    pub lower_error_fn: String,
    pub decl_type_name: String,
}

#[derive(Clone)]
pub(crate) struct TsAsyncFfi {
    pub poll: String,
    pub complete: String,
    pub free: String,
    pub cancel: String,
}

pub(crate) type TsMethod = TsCallable;
pub(crate) type TsConstructor = TsCallable;
pub(crate) type TsFunction = TsCallable;

// ---------------------------------------------------------------------------
// Object IR nodes
// ---------------------------------------------------------------------------

pub(crate) struct TsObject {
    pub ts_name: String,
    pub decl_type_name: String,
    pub impl_class_name: String,
    pub protocol_name: String,
    pub obj_factory: String,
    pub ffi_converter_name: String,
    pub ffi_error_converter_name: String,
    pub docstring: Option<String>,
    pub is_error: bool,
    pub vtable: Option<TsVtable>,
    pub trait_impl: String,
    pub primary_constructor: Option<TsConstructor>,
    pub alternate_constructors: Vec<TsConstructor>,
    pub methods: Vec<TsMethod>,
    pub uniffi_traits: Vec<TsUniffiTrait>,
    pub ffi_bless_pointer: String,
    pub ffi_clone: String,
    pub ffi_free: String,
    pub supports_finalization_registry: bool,
    pub has_callback_interface: bool,
    pub strict_object_types: bool,
}

impl TsObject {
    pub fn has_display_trait(&self) -> bool {
        self.uniffi_traits
            .iter()
            .any(|t| matches!(t, TsUniffiTrait::Display { .. }))
    }
}

pub(crate) enum TsUniffiTrait {
    Display { method: TsMethod },
    Debug { method: TsMethod },
    Eq { eq: TsMethod, ne: Box<TsMethod> },
    Hash { method: TsMethod },
    Ord { cmp: TsMethod },
}

pub(crate) struct TsVtable {
    pub ffi_init_fn: String,
    pub fields: Vec<TsVtableField>,
}

pub(crate) struct TsVtableField {
    pub name: String,
    pub method: Option<TsCallable>,
    pub foreign_future_result: Option<TsForeignFutureResult>,
    /// Closure parameter list, excluding output params (`uniffi_out_return`).
    pub ffi_closure_args: Vec<TsFfiArg>,
    pub has_rust_call_status_arg: bool,
}

pub(crate) struct TsForeignFutureResult {
    pub struct_name: String,
    pub return_ffi_default_value: String,
}

pub(crate) struct TsFfiArg {
    pub name: String,
    pub ffi_type: String,
}

// ---------------------------------------------------------------------------
// Callback interface IR nodes
// ---------------------------------------------------------------------------

/// JS-implemented interface passed to Rust via vtable (opposite of `TsObject`).
pub(crate) struct TsCallbackInterface {
    pub ts_name: String,
    /// Alias for `ts_name`; the duck-typed `object_interface` macro
    /// accesses `obj.protocol_name` for both `TsObject` and `TsCallbackInterface`.
    pub protocol_name: String,
    pub ffi_converter_name: String,
    pub trait_impl: String,
    pub docstring: Option<String>,
    pub methods: Vec<TsCallable>,
    pub vtable: TsVtable,
    pub has_async_methods: bool,
}

pub(crate) struct InitializationIR {
    pub bindings_contract_version: String,
    pub ffi_contract_version_fn: String,
    pub checksums: Vec<TsChecksum>,
    pub initialization_fns: Vec<String>,
}

pub(crate) struct TsChecksum {
    pub raw_name: String,
    pub ffi_fn_name: String,
    pub expected_value: String,
}

/// A single `import { ... } from "path"` statement.
pub(crate) struct TsFileImport {
    pub path: String,
    pub types: Vec<String>,
    pub values: Vec<String>,
}

/// A converter default-import plus destructuring:
/// `import name from "path"; const { ... } = name.converters;`
pub(crate) struct TsConverterImport {
    pub path: String,
    pub default_name: String,
    pub converters: Vec<String>,
}
