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

{%- macro callback_init(module_name, func) %}
{%- call cpp_fn_from_js_decl(func) %} {
    {%- let args = func.arguments() %}
    {%- let arg = args.first().unwrap() %}
    {{ func.name() }}(
        uniffi_jsi::Bridging<{{ arg.type_().borrow()|ffi_type_name_from_js }}>::fromJs(
            rt,
            args[0],
            callInvoker
        )
    );
    return jsi::Value::undefined();
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
uniffi_jsi::Bridging<{{ arg.type_().borrow()|ffi_type_name_from_js }}>::fromJs(rt, args[{{ index }}])
{%- endmacro %}

{%- macro arg_name_from_js(arg, index) -%}
_{{ arg.name() }}_{{ index }}
{%- endmacro %}


{%- macro cpp_func_name(func) %}cpp_{{ func.name() }}{%- endmacro %}

{%- macro callback_fn_decl(callback) %}
    typedef {# space #}
    {%-   match callback.return_type() %}
    {%-     when Some(return_type) %}{{ return_type|ffi_type_name }}
    {%-     when None %}void
    {%-   endmatch %}
    (*{{  callback.name()|ffi_callback_name }})(
    {%-   for arg in callback.arguments() %}
    {{ arg.type_().borrow()|ffi_type_name }} {{ arg.name()|var_name }}{% if !loop.last %}, {% endif %}
    {%-   endfor %}
    {%-   if callback.has_rust_call_status_arg() -%}
    {%      if callback.arguments().len() > 0 %}, {% endif %}RustCallStatus* rust_call_status
    {%-   endif %}
    );
{%- endmacro %}

{%- macro callback_struct_decl(ffi_struct) %}
    typedef struct {{ ffi_struct.name()|ffi_struct_name }} {
    {%- for field in ffi_struct.fields() %}
        {{ field.type_().borrow()|ffi_type_name }} {{ field.name()|var_name }};
    {%- endfor %}
    } {{ ffi_struct.name()|ffi_struct_name }};
{%- endmacro %}
