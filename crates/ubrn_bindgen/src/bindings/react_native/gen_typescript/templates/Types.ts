{%- import "macros.ts" as ts %}
{{- self.import_infra("UniffiInternalError", "errors") -}}
{{- self.import_infra("rustCall", "rust-call") }}

{%- for func in ci.function_definitions() %}
{%- include "TopLevelFunctionTemplate.ts" %}
{%- endfor %}

{%- for type_ in ci.iter_sorted_types() %}
{%- let type_name = type_|type_name(ci) %}
{%- let ffi_converter_name = type_|ffi_converter_name %}
{%- let canonical_type_name = type_|canonical_name %}
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
{%- include "BooleanHelper.ts" %}

{%- when Type::String %}
{%- include "StringHelper.ts" %}

{%- when Type::Bytes %}
{%- include "DataHelper.ts" %}

{%- when Type::Int8 %}
{%- include "Int8Helper.ts" %}

{%- when Type::Int16 %}
{%- include "Int16Helper.ts" %}

{%- when Type::Int32 %}
{%- include "Int32Helper.ts" %}

{%- when Type::Int64 %}
{%- include "Int64Helper.ts" %}

{%- when Type::UInt8 %}
{%- include "UInt8Helper.ts" %}

{%- when Type::UInt16 %}
{%- include "UInt16Helper.ts" %}

{%- when Type::UInt32 %}
{%- include "UInt32Helper.ts" %}

{%- when Type::UInt64 %}
{%- include "UInt64Helper.ts" %}

{%- when Type::Float32 %}
{%- include "Float32Helper.ts" %}

{%- when Type::Float64 %}
{%- include "Float64Helper.ts" %}

{%- when Type::Timestamp %}
{%- include "TimestampHelper.ts" %}

{%- when Type::Duration %}
{%- include "DurationHelper.ts" %}

{%- when Type::CallbackInterface { name, module_path } %}
{%- include "CallbackInterfaceTemplate.ts" %}
{%- when Type::Custom { name, module_path, builtin } %}
{#
    {%- include "CustomType.ts" %}
#}
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
