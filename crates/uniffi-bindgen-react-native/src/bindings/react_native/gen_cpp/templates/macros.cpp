{%- macro rust_fn_decl(func) %}
    {%- match func.return_type() %}
    {%- when Some with (return_type) %}
    {{- return_type.borrow()|ffi_type_name_to_rust }}
    {%- when None %}void
    {%- endmatch %} {# space #}
    {{- func.name() }}(
        {%- for arg in func.arguments() %}
        {{    arg.type_().borrow()|ffi_type_name_to_rust }} {{ arg.name() }}
        {%-   if !loop.last %}, {# space #}
        {%-   endif %}
        {%- endfor %}
        {%- if func.has_rust_call_status_arg() %}
        {%-   if !func.arguments().is_empty() %}, {# space #}
        {%   endif %}RustCallStatus *uniffi_out_err
        {%- endif %}
    );
{%- endmacro %}

{%- macro rust_fn_caller(module_name, func) %}
{%- let func_name = func.name() %}
jsi::Value {{ module_name }}::{% call cpp_func_name(func) %}(jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count) {
{%- call cpp_fn_rust_caller_body(func) %}
}
{%- endmacro %}

{%- macro cpp_fn_rust_caller_body(func) %}
    {%- if func.has_rust_call_status_arg() %}
    RustCallStatus status = { 0 };
    {%- endif %}
    {% if func.return_type().is_some() -%}
    auto value = {# space #}
    {%- endif %}{{ func_name }}(
        {%- for arg in func.arguments() %}
        uniffi_jsi::Bridging<{{ arg.type_().borrow()|ffi_type_name_from_js }}>::fromJs(rt, args[{{ loop.index0 }}])
        {%- if !loop.last %},
        {%- endif %}
        {%- endfor %}
        {%- if func.has_rust_call_status_arg() %}
        {%-   if !func.arguments().is_empty() %}, {# space #}
        {%   endif %}&status
        {%- endif %}
    );
    {%- if func.has_rust_call_status_arg() %}
    // copy call status to js
    {%- endif %}
    {%- match func.return_type() %}
    {%- when Some with (return_type) %}
    return uniffi_jsi::Bridging<{{ return_type.borrow()|ffi_type_name_from_js }}>::toJs(rt, value);
    {%- when None %}
    return jsi::Value::undefined();
    {%- endmatch %}
{% endmacro %}

{%- macro cpp_func_name(func) %}cpp_{{ func.name() }}{%- endmacro %}
