{%- let struct_name = ffi_struct.name()|ffi_struct_name %}
namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<{{ struct_name }}> {
  static {{ struct_name }} fromJs(jsi::Runtime &rt, const jsi::Value &jsValue, std::shared_ptr<react::CallInvoker> callInvoker) {
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
    {%-   let field_name = field.name()|var_name %}
    {%-   let func_name = field_name|fmt("fn_{}") %}
    auto {{ func_name }} = obj.getPropertyAsFunction(rt, "{{ field_name }}");
    {%- endfor %}

    // Create the vtable from the js callbacks.
    {%- for field in ffi_struct.ffi_functions() %}
    {%-   let field_name = field.name()|var_name %}
    {%-   let func_name = field_name|fmt("fn_{}") %}
    {%-   let field_type = field.type_().borrow()|ffi_type_name %}
    {%-   let ns = field_type|lower|fmt("uniffi_jsi::{}") %}
    vtable.{{ field_name }} = {{ ns }}::makeCallbackFunction(rt, callInvoker, std::move({{ func_name }}));
    {%- endfor %}

    return vtable;
  }
};

} // namespace uniffi_jsi
