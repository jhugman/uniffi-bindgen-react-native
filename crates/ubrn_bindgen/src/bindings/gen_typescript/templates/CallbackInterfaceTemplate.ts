{%- let cbi = ci.get_callback_interface_definition(name).expect("Callback Interface definition not found in this ci") %}
{%- let methods = cbi.methods() %}
{%- let protocol_name = type_name.clone() %}
{%- let protocol_docstring = cbi.docstring() %}
{%- let vtable = cbi.vtable() %}
{{- self.import_infra("FfiConverterCallback", "callbacks") }}
{#- obj is used to generate an interface with ObjectInterfaceTemplate.ts #}
{%- let obj = cbi %}
{% include "ObjectInterfaceTemplate.ts" %}
{% include "CallbackInterfaceImpl.ts" %}

// FfiConverter protocol for callback interfaces
const {{ ffi_converter_name }} = new FfiConverterCallback<{{ type_name }}>();
