{%- let struct_name = ffi_struct.name()|ffi_struct_name %}
namespace {{ ci.cpp_namespace() }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<{{ struct_name }}> {
  static {{ struct_name }} fromJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &jsValue
  ) {
    // Check if the input is an object
    if (!jsValue.isObject()) {
      throw jsi::JSError(rt, "Expected an object for {{ struct_name }}");
    }

    // Get the object from the jsi::Value
    auto jsObject = jsValue.getObject(rt);

    // Create the vtable struct
    {{ struct_name }} rsObject;

    // Create the vtable from the js callbacks.
    {%- for field in ffi_struct.fields() %}
    {%-   let rs_field_name = field.name() %}
    {%-   let ts_field_name = field.name()|var_name %}
    {%-   if field.type_().is_callable() %}
    rsObject.{{ rs_field_name }} = {# space #}
    {%-     call cpp::callback_fn_namespace(ffi_struct, field) -%}
        ::makeCallbackFunction(
          rt, callInvoker, jsObject.getProperty(rt, "{{ ts_field_name }}")
        );
    {%-   else %}
    rsObject.{{ rs_field_name }} = {# space -#}
      {{ field.type_().borrow()|bridging_class(ci) }}::fromJs(
        rt, callInvoker,
        jsObject.getProperty(rt, "{{ ts_field_name }}")
      );
    {%-   endif %}
    {%- endfor %}

    return rsObject;
  }
};

} // namespace {{ ci.cpp_namespace() }}
