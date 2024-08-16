{%- import "macros.ts" as ts %}
{{- self.import_infra("UniffiInternalError", "errors") -}}
{{- self.import_infra("rustCall", "rust-call") }}

{%- for func in ci.function_definitions() %}
{%- include "TopLevelFunctionTemplate.ts" %}
{%- endfor %}

{%- for type_ in ci.iter_sorted_types() %}
{%- let type_name = type_|type_name(self) %}
{%- let decl_type_name = type_|decl_type_name(self) %}
{%- let ffi_converter_name = type_|ffi_converter_name(self) %}
{%- let contains_object_references = ci.item_contains_object_references(type_) %}

{#
 # Map `Type` instances to an include statement for that type.
 #
 # There is a companion match in `KotlinCodeOracle::create_code_type()` which performs a similar function for the
 # Rust code.
 #
 #   - When adding additional types here, make sure to also add a match arm to that function.
 #   - To keep things manageable, let's try to limit ourselves to these 2 mega-matches
 #}
{%- match type_ %}

{%- when Type::Boolean %}
{{- self.import_infra("FfiConverterBool", "ffi-converters") }}

{%- when Type::String %}
{%- include "StringHelper.ts" %}

{%- when Type::Bytes %}
{{- self.import_infra("FfiConverterArrayBuffer", "ffi-converters") }}

{%- when Type::Int8 %}
{{- self.import_infra("FfiConverterInt8", "ffi-converters") }}

{%- when Type::Int16 %}
{{- self.import_infra("FfiConverterInt16", "ffi-converters") }}

{%- when Type::Int32 %}
{{- self.import_infra("FfiConverterInt32", "ffi-converters") }}

{%- when Type::Int64 %}
{{- self.import_infra("FfiConverterInt64", "ffi-converters") }}

{%- when Type::UInt8 %}
{{- self.import_infra("FfiConverterUInt8", "ffi-converters") }}

{%- when Type::UInt16 %}
{{- self.import_infra("FfiConverterUInt16", "ffi-converters") }}

{%- when Type::UInt32 %}
{{- self.import_infra("FfiConverterUInt32", "ffi-converters") }}

{%- when Type::UInt64 %}
{{- self.import_infra("FfiConverterUInt64", "ffi-converters") }}

{%- when Type::Float32 %}
{{- self.import_infra("FfiConverterFloat32", "ffi-converters") }}

{%- when Type::Float64 %}
{{- self.import_infra("FfiConverterFloat64", "ffi-converters") }}

{%- when Type::Timestamp %}
{{- self.import_infra("FfiConverterTimestamp", "ffi-converters") -}}
{{- self.import_infra_type("UniffiTimestamp", "ffi-converters") -}}

{%- when Type::Duration %}
{{- self.import_infra("FfiConverterDuration", "ffi-converters") -}}
{{- self.import_infra_type("UniffiDuration", "ffi-converters") -}}

{%- when Type::CallbackInterface { name, module_path } %}
{%- include "CallbackInterfaceTemplate.ts" %}

{%- when Type::Custom { name, module_path, builtin } %}
{%- include "CustomTypeTemplate.ts" %}

{%- when Type::Enum { name, module_path } %}
{%- let e = ci.get_enum_definition(name).unwrap() %}
{%- if ci.is_name_used_as_error(name) %}
{%- include "ErrorTemplate.ts" %}
{%- else %}
{%- include "EnumTemplate.ts" %}
{% endif %}

{%- when Type::External{ name, module_path, namespace, kind, tagged } %}
{%- include "ExternalTemplate.ts" %}

{%- when Type::Object{ name, module_path, imp } %}
{%- include "ObjectTemplate.ts" %}

{%- when Type::Record { name, module_path } %}
{%- include "RecordTemplate.ts" %}

{%- when Type::Optional { inner_type } %}
{%- include "OptionalTemplate.ts" %}

{%- when Type::Sequence { inner_type } %}
{%- include "SequenceTemplate.ts" %}

{%- when Type::Map { key_type, value_type } %}
{%- include "MapTemplate.ts" %}

{%- else %}
{%- endmatch %}
{%- endfor %}

{% include "InitializationTemplate.ts" %}
