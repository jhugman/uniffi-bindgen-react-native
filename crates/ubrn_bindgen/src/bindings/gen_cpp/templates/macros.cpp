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
{%- call cpp_fn_from_js_decl(func) %} {
    {%- call cpp_fn_rust_caller_body(func) %}
}
{%- endmacro %}

{%- macro cpp_fn_from_js_decl(func) -%}
jsi::Value {{ module_name }}::{% call cpp_func_name(func) %}(jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count)
{%- endmacro %}

{%- macro cpp_fn_rust_caller_body(func) %}
{%- call cpp_fn_rust_caller_body_with_func_name(func, func.name()) %}
{%- endmacro %}

{%- macro cpp_fn_rust_caller_body_with_func_name(func, func_name) %}
        {%- if func.has_rust_call_status_arg() %}
        RustCallStatus status = {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::rustSuccess(rt);
        {%- endif %}

        {#- Now call into Rust #}
        {% if func.return_type().is_some() -%}
        auto value = {# space #}
        {%- endif %}
        {{- func_name }}(
            {%- for arg in func.arguments() %}
            {%-   call arg_from_js(arg, loop.index0) %}
            {%-   if !loop.last %}, {# space #}
            {%-   endif %}
            {%- endfor %}
            {%- if func.has_rust_call_status_arg() %}
            {%-   if !func.arguments().is_empty() %}, {# space #}
            {%   endif %}&status
            {%- endif %}
        );

        {#- Now copy the call status into JS. #}
        {%- if func.has_rust_call_status_arg() %}
        {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::copyIntoJs(rt, callInvoker, status, args[count - 1]);
        {%- endif %}

        {# Finally, lift the result value from C into JS. #}
        {%- match func.return_type() %}
        {%- when Some with (return_type) %}
        return {{ return_type.borrow()|bridging_class(ci) }}::toJs(rt, callInvoker, value);
        {%- when None %}
        return jsi::Value::undefined();
        {%- endmatch %}
{%- endmacro %}

{%- macro arg_from_js(arg, index) -%}
{{ arg.type_().borrow()|bridging_class(ci) }}::fromJs(rt, callInvoker, args[{{ index }}])
{%- endmacro %}

{%- macro cpp_func_name(func) %}cpp_{{ func.name() }}{%- endmacro %}

{# CALLBACKS #}

{%- macro callback_init(module_name, func) %}
{%- call cpp_fn_from_js_decl(func) %} {
    {%- let args = func.arguments() %}
    {%- let arg = args.first().unwrap() %}
    auto vtableInstance =
        {{ arg.type_().borrow()|bridging_class(ci) }}::fromJs(
            rt,
            callInvoker,
            args[0]
        );

    std::lock_guard<std::mutex> lock({{ registry }}::vtableMutex);
    {{ func.name() }}(
        {{ registry }}::putTable(
            "{{ arg.type_().borrow()|ffi_type_name_from_js }}",
            vtableInstance
        )
    );
    return jsi::Value::undefined();
}
{%- endmacro %}

{%- macro callback_fn_decl(callback) %}
    typedef {# space #}
    {%-   match callback.return_type() %}
    {%-     when Some(return_type) %}{{ return_type|ffi_type_name }}
    {%-     when None %}void
    {%-   endmatch %}
    (*{{  callback.name()|ffi_callback_name }})(
    {%-   for arg in callback.arguments() %}
    {{ arg.type_().borrow()|ffi_type_name }} {{ arg.name() }}{% if !loop.last %}, {% endif %}
    {%-   endfor %}
    {%-   if callback.has_rust_call_status_arg() -%}
    {%      if callback.arguments().len() > 0 %}, {% endif %}RustCallStatus* rust_call_status
    {%-   endif %}
    );
{%- endmacro %}

{#-
// ns is the namespace used for the callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_impl(callback) %}
{%- let ns = callback.cpp_namespace(ci) %}
{%- include "CallbackFunction.cpp" %}
{%- endmacro %}

{#-
// ns is the namespace used for the free callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_free_impl(callback) %}
{%- for st in self.ci.iter_ffi_structs_for_free() %}
{%- let ns = st.cpp_namespace_free(ci) %}
{%- include "CallbackFunction.cpp" %}
{%- endfor %}
{%- endmacro %}

{#-
// ns is the namespace used for the callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_cleanup(callback) %}
{%- let ns = callback.cpp_namespace(ci) %}
{{- ns }}::cleanup();
{%- endmacro %}

{#-
// ns is the namespace used for the free callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_free_cleanup(callback) %}
{%- for st in self.ci.iter_ffi_structs_for_free() %}
{%- let ns = st.cpp_namespace_free(ci) %}
{{- ns }}::cleanup();
{%- endfor %}
{%- endmacro %}


{%- macro callback_fn_namespace(st, field) %}
{%- if field.is_free() %}
{#- // match the callback_fn_free_impl macro  #}
{{- st.cpp_namespace_free(ci) -}}
{%- else %}
{%- if field.is_user_callback() %}
{#- // user callbacks get a unique ns per vtable struct to avoid rsLambda aliasing  #}
{{- field.cpp_namespace_in_struct(ci, st.name()) }}
{%- else %}
{#- // match the callback_fn_impl macro  #}
{{- field.type_().borrow().cpp_namespace(ci) }}
{%- endif %}
{%- endif %}
{%- endmacro %}

{%- macro callback_fn_vtable_field_cleanup(ffi_struct, field) %}
{%- if field.is_user_callback() %}
{{- field.cpp_namespace_in_struct(ci, ffi_struct.name()) }}::cleanup();
{%- endif %}
{%- endmacro %}

{%- macro callback_struct_decl(ffi_struct) %}
    {%- let struct_name = ffi_struct.name()|ffi_struct_name -%}
    typedef struct {{ struct_name }} {
    {%- for field in ffi_struct.fields() %}
        {{ field.type_().borrow()|ffi_type_name }} {{ field.name() }};
    {%- endfor %}
    } {{ struct_name }};
{%- endmacro %}
