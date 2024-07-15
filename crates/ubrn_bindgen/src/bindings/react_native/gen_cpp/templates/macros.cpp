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
        {%- if func.has_rust_call_status_arg() %}
        RustCallStatus status = { 0 };
        {%- endif %}

        {#- Before the call, make variables out of the args that will need cleanup after the call. #}
        {%- for arg in func.arguments() %}
        {%-   if arg.type_().requires_argument_cleanup() %}
        auto {% call arg_name_from_js(arg, loop.index0) %} = {% call arg_from_js(arg, loop.index0) %};
        {%-   endif %}
        {%- endfor %}

        {#- Now call into Rust #}
        {% if func.return_type().is_some() -%}
        auto value = {# space #}
        {%- endif %}{{ func.name() }}(
            {%- for arg in func.arguments() %}
            {%    if arg.type_().requires_argument_cleanup() %}
            {%-     call arg_name_from_js(arg, loop.index0) %}
            {%-   else %}
            {%-     call arg_from_js(arg, loop.index0) %}
            {%-   endif %}
            {%-   if !loop.last %}, {# space #}
            {%-   endif %}
            {%- endfor %}
            {%- if func.has_rust_call_status_arg() %}
            {%-   if !func.arguments().is_empty() %}, {# space #}
            {%   endif %}&status
            {%- endif %}
        );

        {#- Now the call is done, we can cleanup all arguments that need it. #}
        {%- for arg in func.arguments() %}
        {%-   if arg.type_().requires_argument_cleanup() %}
        uniffi_jsi::Bridging<{{ arg.type_().borrow()|ffi_type_name_from_js }}>::argument_cleanup(rt, {% call arg_name_from_js(arg, loop.index0) %});
        {%-   endif %}
        {%- endfor %}

        {#- Now copy the call status into JS. #}
        {%- if func.has_rust_call_status_arg() %}
        uniffi_jsi::Bridging<RustCallStatus>::copyIntoJs(rt, status, args[count - 1]);
        {%- endif %}

        {# Finally, lift the result value from C into JS. #}
        {%- match func.return_type() %}
        {%- when Some with (return_type) %}
        return uniffi_jsi::Bridging<{{ return_type.borrow()|ffi_type_name_from_js }}>::toJs(rt, value);
        {%- when None %}
        return jsi::Value::undefined();
        {%- endmatch %}
{%- endmacro %}

{%- macro arg_from_js(arg, index) -%}
uniffi_jsi::Bridging<{{ arg.type_().borrow()|ffi_type_name_from_js }}>::fromJs(rt, callInvoker, args[{{ index }}])
{%- endmacro %}

{%- macro arg_name_from_js(arg, index) -%}
_{{ arg.name() }}_{{ index }}
{%- endmacro %}


{%- macro cpp_func_name(func) %}cpp_{{ func.name() }}{%- endmacro %}

{# CALLBACKS #}

{%- macro callback_init(module_name, func) %}
{%- call cpp_fn_from_js_decl(func) %} {
    {%- let args = func.arguments() %}
    {%- let arg = args.first().unwrap() %}
    {%- let vtable_t = arg.type_().borrow()|ffi_type_name_from_js %}
    static {{ vtable_t }} vtableInstance =
        uniffi_jsi::Bridging<{{ vtable_t }}>::fromJs(
            rt,
            callInvoker,
            args[0]
        );
    {{ func.name() }}(&vtableInstance);
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
{%- let ns = callback.name()|ffi_callback_name|lower|fmt("uniffi_jsi::{}") %}
{%- include "VTableCallbackFunction.cpp" %}
{%- endmacro %}

{#-
// ns is the namespace used for the free callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_free_impl(callback) %}
{%- call callback_fn_impl(callback) %}
{%- for st in self.ci.iter_ffi_structs() %}
{%- let ns = st.name()|lower|fmt("uniffi_jsi::{}::freecallback") %}
{%- include "VTableCallbackFunction.cpp" %}
{%- endfor %}
{%- endmacro %}

{#-
// ns is the namespace used for the callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_cleanup(callback) %}
{%- let ns = callback.name()|ffi_callback_name|lower|fmt("uniffi_jsi::{}") %}
{{- ns }}::cleanup();
{%- endmacro %}

{#-
// ns is the namespace used for the free callback function.
// It should match the value rendered by the callback_fn_namespace macro.
#}
{%- macro callback_fn_free_cleanup(callback) %}
{%- call callback_fn_cleanup(callback) %}
{%- for st in self.ci.iter_ffi_structs() %}
{%- let ns = st.name()|lower|fmt("uniffi_jsi::{}::freecallback") %}
{{- ns }}::cleanup();
{%- endfor %}
{%- endmacro %}


{%- macro callback_fn_namespace(st, field) %}
{%- if field.is_free() %}
{#- // match the callback_fn_free_impl macro  #}
{{- st.name()|lower|fmt("uniffi_jsi::{}::freecallback") -}}
{%- else %}
{#- // match the callback_fn_impl macro  #}
{%- let field_type = field.type_().borrow()|ffi_type_name %}
{{- field_type|lower|fmt("uniffi_jsi::{}")}}
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
