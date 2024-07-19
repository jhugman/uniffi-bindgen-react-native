{%- import "macros.cpp" as cpp %}

// Utility functions for serialization/deserialization of strings.
{% let func = ci.ffi_function_string_to_bytelength() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::string_to_bytelength(rt, args[0]);
}

{% let func = ci.ffi_function_string_to_arraybuffer() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::string_to_arraybuffer(rt, args[0]);
}

{% let func = ci.ffi_function_arraybuffer_to_string() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::arraybuffer_to_string(rt, args[0]);
}
