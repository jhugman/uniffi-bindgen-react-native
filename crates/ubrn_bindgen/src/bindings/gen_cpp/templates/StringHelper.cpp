{%- import "macros.cpp" as cpp %}

// Utility functions for serialization/deserialization of strings.
{% let func = ci.ffi_function_string_to_bytelength() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::string_to_bytelength(rt, args[0]);
}

{% let func = ci.ffi_function_string_to_buffer() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::string_to_buffer(rt, args[0]);
}

{% let func = ci.ffi_function_string_from_buffer() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::string_from_buffer(rt, args[0]);
}

{% let func = ci.ffi_function_read_string_from_buffer() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return {{ ci.cpp_namespace_includes() }}::Bridging<std::string>::read_string_from_buffer(rt, args[0], args[1], args[2]);
}
