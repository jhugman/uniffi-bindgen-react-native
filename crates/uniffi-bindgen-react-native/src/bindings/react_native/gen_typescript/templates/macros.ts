{#
    // Template to defining types. This may endup using codegen,
    // but may not.
    // Variable names in `arg_list_decl` should match up with arg lists
    // passed to rust via `arg_list_lowered`
    #}

{%- macro arg_list_ffi_decl(func) %}
    {%- for arg in func.arguments() %}
        {{- arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name_by_value }}
        {%- if !loop.last %}, {% endif %}
    {%- endfor %}
    {%- if func.has_rust_call_status_arg() %}
    {%- if !func.arguments().is_empty() %}, {% endif -%}
    uniffi_out_err: UniffiRustCallStatus{% endif %}
{%- endmacro %}

{#
// Template to call into rust. Used in several places.
// Variable names in `arg_list_decl` should match up with arg lists
// passed to rust via `arg_list_lowered`
#}


{%- macro to_ffi_call(func) -%}
    {%- match func.throws_type() -%}
    {%- when Some with (e) -%}
        rustCallWithError({{ e|ffi_error_converter_name }}.lift, callStatus => {
    {%- else -%}
        rustCall(callStatus => {
    {%- endmatch %}
    {%- if func.return_type().is_some() %}
        return
    {%- endif %} NativeModule.{{ func.ffi_func().name() }}(
        {%- if func.takes_self() %}this.uniffiClonePointer(), {% endif %}
        {%- call arg_list_lowered(func) %}
        callStatus);
    })
{%- endmacro -%}

// eg, `public func foo_bar() { body }`
{%- macro func_decl(func_decl, callable, indent, export) %}
{%- call docstring(callable, indent) %}
{{ export }}{% call async(callable) %}{{ func_decl }} {{ callable.name()|fn_name }}(
    {%- call arg_list_decl(callable) -%})

    {%- call returns(callable) %}
    {%- call throws(callable) %} {
    {%- call call_body(callable) %}
    }
{%- endmacro %}

{%- macro returns(callable) %}
    {%- match callable.return_type() %}
    {%-  when Some with (return_type) %}: {% if callable.is_async() %}Promise<{{ return_type|type_name(ci) }}>{% else %}{{ return_type|type_name(ci) }}{% endif %}
    {%-  when None %}
    {%- endmatch %}
{%- endmacro %}

{%- macro return_type_name(callable) %}
    {%- match callable.return_type() %}
    {%-  when Some with (return_type) %}{{ return_type|type_name(ci) }}
    {%-  when None %}()
    {%- endmatch %}
{%- endmacro %}

// primary ctor - no name, no return-type.
{%- macro ctor_decl(callable, indent) %}
{%- call docstring(callable, indent) %}
    constructor(
    {%- call arg_list_decl(callable) -%}) {%- call async(callable) %} {%- call throws(callable) %} {
    {%- if callable.is_async() %}
        const pointer =
            {%- call call_async(callable) %}
            {# The async mechanism returns an already constructed self.
            We work around that by cloning the pointer from that object, then
            assune the old object dies as there are no other references possible.
            #}
            .uniffiClonePointer()
        {%- else %}
        this.pointer =
            {% call to_ffi_call(callable) %}
    {%- endif %}
    }
{%- endmacro %}

{%- macro call_body(callable) %}
{%- if callable.is_async() %}
    return {%- call call_async(callable) %};
{%- else %}
{%-     match callable.return_type() -%}
{%-         when Some with (return_type) %}
    return {{ return_type|lift_fn }}({% call to_ffi_call(callable) %});
{%-         when None %}
{%-             call to_ffi_call(callable) %};
{%-     endmatch %}
{%- endif %}

{%- endmacro %}

{%- macro call_async(callable) %}
        {% call try(callable) %} await uniffiRustCallAsync(
            rustFutureFunc: {
                {{ callable.ffi_func().name() }}(
                    {%- if callable.takes_self() %}
                    self.uniffiClonePointer(){% if !callable.arguments().is_empty() %},{% endif %}
                    {% endif %}
                    {%- for arg in callable.arguments() -%}
                    {{ arg|lower_fn }}({{ arg.name()|var_name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                )
            },
            pollFunc: {{ callable.ffi_rust_future_poll(ci) }},
            completeFunc: {{ callable.ffi_rust_future_complete(ci) }},
            freeFunc: {{ callable.ffi_rust_future_free(ci) }},
            {%- match callable.return_type() %}
            {%- when Some(return_type) %}
            liftFunc: {{ return_type|lift_fn }},
            {%- when None %}
            liftFunc: { $0 },
            {%- endmatch %}
            {%- match callable.throws_type() %}
            {%- when Some with (e) %}
            errorHandler: {{ e|ffi_error_converter_name }}.lift
            {%- else %}
            errorHandler: nil
            {% endmatch %}
        )
{%- endmacro %}

{%- macro arg_list_lowered(func) %}
    {%- for arg in func.arguments() %}
        {{ arg|lower_fn }}({{ arg.name()|var_name }}),
    {%- endfor %}
{%- endmacro -%}

{#-
// Arglist as used in ts declarations of methods, functions and constructors.
// Note the var_name and type_name filters.
-#}

{% macro arg_list_decl(func) %}
    {%- for arg in func.arguments() -%}
        {{ arg.name()|var_name }}: {{ arg|type_name(ci) -}}
        {%- match arg.default_value() %}
        {%- when Some with(literal) %} = {{ literal|render_literal(arg, ci) }}
        {%- else %}
        {%- endmatch %}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
{%- endmacro %}

{#-
// Field lists as used in ts declarations of Records and Enums.
// Note the var_name and type_name filters.
-#}
{% macro field_list_decl(item, has_nameless_fields) %}
    {%- for field in item.fields() -%}
        {%- call docstring(field, 8) %}
        {%- if has_nameless_fields %}
        {{- field|type_name(ci) -}}
        {%- if !loop.last -%}, {%- endif -%}
        {%- else -%}
        {{ field.name()|var_name }}: {{ field|type_name(ci) -}}
        {%- match field.default_value() %}
            {%- when Some with(literal) %} = {{ literal|render_literal(field, ci) }}
            {%- else %}
        {%- endmatch -%}
        {% if !loop.last %}, {% endif %}
        {%- endif -%}
    {%- endfor %}
{%- endmacro %}

{% macro field_name(field, field_num) %}
{%- if field.name().is_empty() -%}
v{{- field_num -}}
{%- else -%}
{{ field.name()|var_name }}
{%- endif -%}
{%- endmacro %}

{% macro arg_list_protocol(func) %}
    {%- for arg in func.arguments() -%}
        {{ arg.name()|var_name }}: {{ arg|type_name(ci) -}}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
{%- endmacro %}

{%- macro async(func) %}
{%- if func.is_async() %}async {% endif %}
{%- endmacro -%}

{%- macro throws(func) %}
{%- if func.throws() %} /*throws*/{% endif %}
{%- endmacro -%}

{%- macro try(func) %}
{%- if func.throws() %}/*try*/ {% else %}/*try!*/ {% endif %}
{%- endmacro -%}

{%- macro docstring_value(maybe_docstring, indent_spaces) %}
{%- match maybe_docstring %}
{%- when Some(docstring) %}
{{ docstring|docstring(indent_spaces) }}
{%- else %}
{%- endmatch %}
{%- endmacro %}

{%- macro docstring(defn, indent_spaces) %}
{%- call docstring_value(defn.docstring(), indent_spaces) %}
{%- endmacro %}
