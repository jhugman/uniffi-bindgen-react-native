{%- let struct_name = ffi_struct.name()|ffi_struct_name %}
namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<{{ struct_name }}> {
  static {{ struct_name }} fromJs(jsi::Runtime &rt, const jsi::Value &jsValue, std::shared_ptr<uniffi_runtime::UniffiCallInvoker> callInvoker) {
    // Check if the input is an object
    if (!jsValue.isObject()) {
      throw jsi::JSError(rt, "Expected an object for {{ struct_name }}");
    }

    // Get the object from the jsi::Value
    auto obj = jsValue.getObject(rt);

    // Create the vtable struct
    {{ struct_name }} vtable;

    // Extract the function callbacks from the JS object
    {%- for field in ffi_struct.ffi_functions() %}
    {%-   let ts_field_name = field.name()|var_name %}
    {%-   let func_name = ts_field_name|fmt("fn_{}") %}
    auto {{ func_name }} = obj.getPropertyAsFunction(rt, "{{ ts_field_name }}");
    {%- endfor %}

    // Create the vtable from the js callbacks.
    {%- for field in ffi_struct.ffi_functions() %}
    {%-   let rs_field_name = field.name() %}
    {%-   let func_name = rs_field_name|var_name|fmt("fn_{}") %}
    vtable.{{ rs_field_name }} = {# space #}
      {%- call cpp::callback_fn_namespace(ffi_struct, field) -%}
        ::makeCallbackFunction(rt, callInvoker, {{ func_name }});
    {%- endfor %}

    return vtable;
  }
};

} // namespace uniffi_jsi
