{#- Callback interface template (v2, IR-driven).

    Expected renderer fields:
    - `cbi: &TsCallbackInterface`
    - `is_verbose: &bool`
    - `console_import: &'a Option<String>`
-#}
{%- let ffi_converter_name = cbi.ffi_converter_name %}
{%- let trait_impl = cbi.trait_impl %}
{%- let vtable = cbi.vtable %}

{#- Render the protocol interface using the same template as objects -#}
{%- let obj = cbi %}
{% include "ObjectInterfaceTemplate.ts" %}

{#- Include the vtable implementation -#}
{% include "CallbackInterfaceImpl.ts" %}

// FfiConverter protocol for callback interfaces
const {{ cbi.ffi_converter_name }} = new FfiConverterCallback<{{ cbi.ts_name }}>();
