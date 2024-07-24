/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToUpperCamelCase};
use uniffi_bindgen::{
    backend::{Literal, Type},
    interface::{AsType, FfiType, Object},
    ComponentInterface,
};

use super::*;

pub(crate) struct CodeOracle;

impl CodeOracle {
    pub(crate) fn find(&self, type_: &Type) -> Box<dyn CodeType> {
        type_.clone().as_type().as_codetype()
    }

    /// Get the idiomatic Typescript rendering of a class name (for enums, records, errors, etc).
    pub(crate) fn class_name(&self, _ci: &ComponentInterface, nm: &str) -> String {
        nm.to_string().to_upper_camel_case()
    }

    pub(crate) fn convert_error_suffix(&self, nm: &str) -> String {
        match nm.strip_suffix("Error") {
            None => nm.to_string(),
            Some(stripped) => format!("{stripped}Exception"),
        }
    }

    /// Get the idiomatic Typescript rendering of a function name.
    pub(crate) fn fn_name(&self, nm: &str) -> String {
        if nm == "new" {
            "create".to_string()
        } else {
            rewrite_keywords(nm.to_string().to_lower_camel_case())
        }
    }

    /// Get the idiomatic Typescript rendering of a variable name.
    pub(crate) fn var_name(&self, nm: &str) -> String {
        rewrite_keywords(nm.to_string().to_lower_camel_case())
    }

    /// Get the idiomatic Typescript rendering of an individual enum variant.
    pub(crate) fn enum_variant_name(&self, nm: &str) -> String {
        nm.to_string().to_shouty_snake_case()
    }

    /// Get the idiomatic Typescript rendering of an FFI callback function name
    pub(crate) fn ffi_callback_name(&self, nm: &str) -> String {
        format!("Uniffi{}", nm.to_upper_camel_case())
    }

    /// Get the idiomatic Typescript rendering of an FFI struct name
    pub(crate) fn ffi_struct_name(&self, nm: &str) -> String {
        format!("Uniffi{}", nm.to_upper_camel_case())
    }

    pub(crate) fn ffi_type_label_by_value(&self, ffi_type: &FfiType) -> String {
        // match ffi_type {
        // FfiType::RustBuffer(_) => format!("{}.ByValue", self.ffi_type_label(ffi_type)),
        // FfiType::Struct(name) => format!("{}.UniffiByValue", self.ffi_struct_name(name)),
        // _ => self.ffi_type_label(ffi_type),
        // }
        self.ffi_type_label(ffi_type)
    }

    /// FFI type name to use inside structs
    ///
    /// The main requirement here is that all types must have default values or else the struct
    /// won't work in some JNA contexts.
    #[allow(unused)]
    pub(crate) fn ffi_type_label_for_ffi_struct(&self, ffi_type: &FfiType) -> String {
        match ffi_type {
            // Make callbacks function pointers nullable. This matches the semantics of a C
            // function pointer better and allows for `null` as a default value.
            FfiType::Callback(name) => format!("{}?", self.ffi_callback_name(name)),
            _ => self.ffi_type_label_by_value(ffi_type),
        }
    }

    /// Default values for FFI
    ///
    /// This is used to:
    ///   - Set a default return value for error results
    ///   - Set a default for structs, which JNA sometimes requires
    pub(crate) fn ffi_default_value(&self, ffi_type: &FfiType) -> String {
        match ffi_type {
            FfiType::UInt8
            | FfiType::Int8
            | FfiType::UInt16
            | FfiType::Int16
            | FfiType::UInt32
            | FfiType::Int32 => "0".to_owned(),
            FfiType::UInt64 | FfiType::Int64 => "0n".to_owned(),
            FfiType::Float64 => "0.0".to_owned(),
            FfiType::Float32 => "0.0".to_owned(),
            FfiType::RustArcPtr(_) => "null".to_owned(),
            FfiType::RustBuffer(_) => "/*empty*/ new ArrayBuffer(0)".to_owned(),
            FfiType::Callback(_) => "null".to_owned(),
            FfiType::RustCallStatus => "uniffiCreateCallStatus()".to_owned(),
            _ => unimplemented!("ffi_default_value: {ffi_type:?}"),
        }
    }

    pub(crate) fn ffi_type_label_by_reference(&self, ffi_type: &FfiType) -> String {
        match ffi_type {
            FfiType::Int8
            | FfiType::UInt8
            | FfiType::Int16
            | FfiType::UInt16
            | FfiType::Int32
            | FfiType::UInt32
            | FfiType::Int64
            | FfiType::UInt64
            | FfiType::Float32
            | FfiType::Float64
            | FfiType::RustBuffer(_) => {
                format!("UniffiReferenceHolder<{}>", self.ffi_type_label(ffi_type))
            }
            FfiType::Struct(nm) if nm.starts_with("VTableCallbackInterface") => {
                self.ffi_type_label(ffi_type)
            }
            FfiType::Struct(_) => {
                format!("UniffiReferenceHolder<{}>", self.ffi_type_label(ffi_type))
            }
            FfiType::RustArcPtr(_) => "PointerByReference".to_owned(),
            // JNA structs default to ByReference
            _ => panic!("{ffi_type:?} by reference is not implemented"),
        }
    }

    pub(crate) fn ffi_type_label_for_cpp(&self, ffi_type: &FfiType) -> String {
        match ffi_type {
            FfiType::ForeignBytes => "ArrayBuffer".to_string(),
            FfiType::RustBuffer(_) => "string".to_string(),
            _ => self.ffi_type_label(ffi_type),
        }
    }

    pub(crate) fn ffi_type_label(&self, ffi_type: &FfiType) -> String {
        match ffi_type {
            FfiType::Int8 | FfiType::UInt8 => "number".to_string(),
            FfiType::Int16 | FfiType::UInt16 => "number".to_string(),
            FfiType::Int32 | FfiType::UInt32 => "number".to_string(),
            FfiType::Int64 | FfiType::UInt64 => "bigint".to_string(),
            FfiType::Float32 => "number".to_string(),
            FfiType::Float64 => "number".to_string(),
            FfiType::Handle => "bigint".to_string(),
            FfiType::RustArcPtr(_) => "bigint".to_string(),
            FfiType::RustBuffer(_) => "ArrayBuffer".to_string(),
            FfiType::RustCallStatus => "UniffiRustCallStatus".to_string(),
            FfiType::ForeignBytes => "ForeignBytes".to_string(),
            FfiType::Callback(name) => self.ffi_callback_name(name),
            FfiType::Struct(name) => self.ffi_struct_name(name),
            FfiType::Reference(inner) => self.ffi_type_label_by_reference(inner),
            FfiType::VoidPointer => "/*pointer*/ bigint".to_string(),
        }
    }

    /// Get the name of the interface and class name for an object.
    ///
    /// If we support callback interfaces, the interface name is the object name, and the class name is derived from that.
    /// Otherwise, the class name is the object name and the interface name is derived from that.
    ///
    /// This split determines what types `FfiConverter.lower()` inputs.  If we support callback
    /// interfaces, `lower` must lower anything that implements the interface.  If not, then lower
    /// only lowers the concrete class.
    pub(crate) fn object_names(&self, ci: &ComponentInterface, obj: &Object) -> (String, String) {
        let class_name = self.class_name(ci, obj.name());
        if obj.has_callback_interface() {
            let impl_name = format!("{class_name}Impl");
            (class_name, impl_name)
        } else {
            (format!("{class_name}Interface"), class_name)
        }
    }
}

pub(crate) trait AsCodeType {
    fn as_codetype(&self) -> Box<dyn CodeType>;
}

impl<T: AsType> AsCodeType for T {
    fn as_codetype(&self) -> Box<dyn CodeType> {
        // Map `Type` instances to a `Box<dyn CodeType>` for that type.
        //
        // There is a companion match in `templates/Types.kt` which performs a similar function for the
        // template code.
        //
        //   - When adding additional types here, make sure to also add a match arm to the `Types.kt` template.
        //   - To keep things manageable, let's try to limit ourselves to these 2 mega-matches
        match self.as_type() {
            Type::UInt8 => Box::new(primitives::UInt8CodeType),
            Type::Int8 => Box::new(primitives::Int8CodeType),
            Type::UInt16 => Box::new(primitives::UInt16CodeType),
            Type::Int16 => Box::new(primitives::Int16CodeType),
            Type::UInt32 => Box::new(primitives::UInt32CodeType),
            Type::Int32 => Box::new(primitives::Int32CodeType),
            Type::UInt64 => Box::new(primitives::UInt64CodeType),
            Type::Int64 => Box::new(primitives::Int64CodeType),
            Type::Float32 => Box::new(primitives::Float32CodeType),
            Type::Float64 => Box::new(primitives::Float64CodeType),
            Type::Boolean => Box::new(primitives::BooleanCodeType),
            Type::String => Box::new(primitives::StringCodeType),
            Type::Bytes => Box::new(primitives::BytesCodeType),

            Type::Timestamp => Box::new(miscellany::TimestampCodeType),
            Type::Duration => Box::new(miscellany::DurationCodeType),

            Type::Enum { name, .. } => Box::new(enum_::EnumCodeType::new(name)),
            Type::Object { name, imp, .. } => Box::new(object::ObjectCodeType::new(name, imp)),
            Type::Record { name, .. } => Box::new(record::RecordCodeType::new(name)),
            Type::CallbackInterface { name, .. } => {
                Box::new(callback_interface::CallbackInterfaceCodeType::new(name))
            }
            Type::Optional { inner_type } => {
                Box::new(compounds::OptionalCodeType::new(*inner_type))
            }
            Type::Sequence { inner_type } => {
                Box::new(compounds::SequenceCodeType::new(*inner_type))
            }
            Type::Map {
                key_type,
                value_type,
            } => Box::new(compounds::MapCodeType::new(*key_type, *value_type)),
            Type::External { name, .. } => Box::new(external::ExternalCodeType::new(name)),
            Type::Custom { name, .. } => Box::new(custom::CustomCodeType::new(name)),
        }
    }
}

/// A trait tor the implementation.
pub(crate) trait CodeType: std::fmt::Debug {
    /// The language specific label used to reference this type. This will be used in
    /// method signatures and property declarations.
    fn type_label(&self, ci: &ComponentInterface) -> String;

    /// The container type for this type. Most of the time, this is the samne as the type_label.
    /// However, just occassionally the typescript type is different.
    /// e.g. errors are instantiated with `new MyError.Foo()`, but have typescript type of
    /// `MyErrorType`.
    fn decl_type_label(&self, ci: &ComponentInterface) -> String {
        self.type_label(ci)
    }

    /// A representation of this type label that can be used as part of another
    /// identifier. e.g. `read_foo()`, or `FooInternals`.
    ///
    /// This is especially useful when creating specialized objects or methods to deal
    /// with this type only.
    fn canonical_name(&self) -> String;

    fn literal(&self, _literal: &Literal, ci: &ComponentInterface) -> String {
        unimplemented!("Unimplemented for {}", self.type_label(ci))
    }

    /// Name of the FfiConverter
    ///
    /// This is the object that contains the lower, write, lift, and read methods for this type.
    fn ffi_converter_name(&self) -> String {
        format!("FfiConverter{}", self.canonical_name())
    }

    // XXX - the below should be removed and replace with the ffi_converter_name reference in the template.
    /// An expression for lowering a value into something we can pass over the FFI.
    fn lower(&self) -> String {
        format!("{}.lower", self.ffi_converter_name())
    }

    /// An expression for writing a value into a byte buffer.
    fn write(&self) -> String {
        format!("{}.write", self.ffi_converter_name())
    }

    /// An expression for lifting a value from something we received over the FFI.
    fn lift(&self) -> String {
        format!("{}.lift", self.ffi_converter_name())
    }

    /// An expression for reading a value from a byte buffer.
    fn read(&self) -> String {
        format!("{}.read", self.ffi_converter_name())
    }

    /// A list of imports that are needed if this type is in use.
    /// Classes are imported exactly once.
    fn imports(&self) -> Option<Vec<String>> {
        None
    }

    /// Function to run at startup
    fn initialization_fn(&self) -> Option<String> {
        None
    }
}

// Note: all keywords are lowercase, so we only need to rewrite identifiers that
// could be all lowercase; i.e. we shouldn't need to re-write upper camel case
// or screaming snake case.
fn rewrite_keywords(nm: String) -> String {
    // Keywords from https://github.com/microsoft/TypeScript/issues/2536#issuecomment-87194347
    let keywords = HashSet::<_>::from([
        // Reserved words in typescript
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "new",
        "null",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        // Strict Mode reserved words in typescript
        "as",
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
    ]);
    if keywords.contains(nm.as_str()) {
        format!("{}_", nm)
    } else {
        nm
    }
}
