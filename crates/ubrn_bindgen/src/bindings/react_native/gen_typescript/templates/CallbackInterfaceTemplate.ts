{%- let cbi = ci|get_callback_interface_definition(name) %}
{%- let callback_handler = format!("uniffiCallbackHandler{}", name) %}
{%- let callback_init = format!("uniffiCallbackInit{}", name) %}
{%- let methods = cbi.methods() %}
{%- let protocol_name = type_name.clone() %}
{%- let protocol_docstring = cbi.docstring() %}
{%- let vtable = cbi.vtable() %}
{%- let vtable_methods = cbi.vtable_methods() %}
{%- let ffi_init_callback = cbi.ffi_init_callback() %}
{{- self.import_infra("FfiConverterCallback", "callbacks") }}
{#- obj is used to generate an interface with ObjectInterfaceTemplate.ts #}
{%- let obj = cbi %}
{% include "ObjectInterfaceTemplate.ts" %}
{% include "CallbackInterfaceImpl.ts" %}

// FfiConverter protocol for callback interfaces
const {{ ffi_converter_name }} = new FfiConverterCallback<{{ type_name }}>();
