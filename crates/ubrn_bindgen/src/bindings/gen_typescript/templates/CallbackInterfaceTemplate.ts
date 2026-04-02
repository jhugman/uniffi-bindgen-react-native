{%- import "ObjectInterfaceTemplate.ts" as oi %}
{%- import "CallbackInterfaceImpl.ts" as cbi_impl %}
{%- macro callback_interface(cbi) %}
{% call oi::object_interface(cbi) %}

{% call cbi_impl::callback_interface_impl(cbi.vtable, cbi.ffi_converter_name, cbi.trait_impl) %}

// FfiConverter protocol for callback interfaces
const {{ cbi.ffi_converter_name }} = new FfiConverterCallback<{{ cbi.ts_name }}>();
{%- endmacro %}
