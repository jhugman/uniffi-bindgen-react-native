{%- import "macros.cpp" as cpp %}

// Utility functions to tie together hermes GC with Rust dropping.
{% let func = ci.ffi_function_bless_pointer() %}
{%- call cpp::cpp_fn_from_js_decl(func) %} {
    return uniffi_jsi::Bridging<void *>::bless_pointer(rt, args[0], args[1]);
}
