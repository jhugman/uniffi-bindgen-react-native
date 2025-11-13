{%- let cb_name = callback.name()|ffi_callback_name %}
{%- let guard_name = cb_name|fmt("WRAPPER_DECL_{}") %}
#ifndef {{ guard_name }}_DEFINED
#define {{ guard_name }}_DEFINED
namespace {{ ci.cpp_namespace() }} {
// Forward declaration of wrapper struct
struct {{ cb_name }}Wrapper {
    {{ cb_name }} callback;
    explicit {{ cb_name }}Wrapper({{ cb_name }} cb) : callback(cb) {}
    operator {{ cb_name }}() const { return callback; }
};
} // namespace {{ ci.cpp_namespace() }}
#endif // {{ guard_name }}_DEFINED


