{#
    // Template to defining types. This may endup using codegen,
    // but may not.
    // Variable names in `arg_list_decl` should match up with arg lists
    // passed to rust via `arg_list_lowered`
    #}

{%- macro arg_list_ffi_decl(func) %}
    {%- let is_internal = func.is_internal() %}
    {%- for arg in func.arguments() %}
        {{- arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name_for_cpp(is_internal) }}
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
{%- call to_ffi_method_call("unreachable", func) %}
{%- endmacro %}

{%- macro to_ffi_method_call(obj_factory, func) -%}
    {%- match func.throws_type() -%}
    {%- when Some with (e) -%}
        rustCallWithError({{ e|lift_fn }}, callStatus => {
    {%- else -%}
        rustCall(callStatus => {
    {%- endmatch %}
    {%- if func.return_type().is_some() %}
        return
    {%- endif %} nativeModule().{{ func.ffi_func().name() }}(
        {%- if func.takes_self() %}{{ obj_factory }}.clonePointer(this), {% endif %}
        {%- call arg_list_lowered(func) %}
        callStatus);
    })
{%- endmacro -%}

// eg. `export function fooBar() { body }`
{%- macro top_func_decl(func_decl, callable, indent) %}
{%- call func_decl("export ", func_decl, "unreachable", callable, indent) %}
{%- endmacro %}

// e.g. `fooBar() { body }`, which accepts an obj_factory to create, clone and free
// pointers.
{%- macro method_decl(func_decl, obj_factory, callable, indent) %}
{%- call func_decl("", func_decl, obj_factory, callable, indent) %}
{%- endmacro %}

// Internal macro common to method_decl and top_func_decl
{%- macro func_decl(prefix, func_decl, obj_factory, callable, indent) %}
{%- call docstring(callable, indent) %}
{{ prefix }}{% call async(callable) %}{{ func_decl }} {{ callable.name()|fn_name }}(
    {%- call arg_list_decl(callable) -%}): {# space #}

    {%- call return_type(callable) %}
    {%- call throws(callable) %} {
    {%- call call_body(obj_factory, callable) %}
    }
{%- endmacro %}

{%- macro return_type(callable) %}
    {%- match callable.return_type() %}
    {%-  when Some with (return_type) %}{% if callable.is_async() %}Promise<{{ return_type|type_name(ci) }}>{% else %}{{ return_type|type_name(ci) }}{% endif %}
    {%-  when None %}{% if callable.is_async() %}Promise<void>{% else %}void{% endif %}
    {%- endmatch %}
{%- endmacro %}

// primary ctor - no name, no return-type.
{%- macro ctor_decl(obj_factory, callable, indent) %}
{%- call docstring(callable, indent) %}
    constructor(
    {%- call arg_list_decl(callable) -%}) {%- call async(callable) %} {%- call throws(callable) %} {
    {%- if callable.is_async() %}
        const pointer =
            {%- call call_async(obj_factory, callable) %}
            {# The async mechanism returns an already constructed self.
            We work around that by cloning the pointer from that object, then
            assune the old object dies as there are no other references possible.
            #}
            .uniffiClonePointer()
        {%- else %}
        super();
        const pointer =
            {% call to_ffi_method_call(obj_factory, callable) %};
        this._rustArcPtr = {{ obj_factory }}.bless(pointer);
    {%- endif %}
    }
{%- endmacro %}

{%- macro call_body(obj_factory, callable) %}
{%- if callable.is_async() %}
    return {# space #}{%- call call_async(obj_factory, callable) %};
{%- else %}
{%-     match callable.return_type() -%}
{%-         when Some with (return_type) %}
    return {{ return_type|lift_fn }}({% call to_ffi_method_call(obj_factory, callable) %});
{%-         when None %}
{%-             call to_ffi_method_call(obj_factory, callable) %};
{%-     endmatch %}
{%- endif %}

{%- endmacro %}

{%- macro call_async(obj_factory, callable) -%}
        await uniffiRustCallAsync({
            /*rustFutureFunc:*/ () => {
                return nativeModule().{{ callable.ffi_func().name() }}(
                    {%- if callable.takes_self() %}
                    {{ obj_factory }}.clonePointer(this){% if !callable.arguments().is_empty() %},{% endif %}
                    {% endif %}
                    {%- for arg in callable.arguments() -%}
                    {{ arg|lower_fn }}({{ arg.name()|var_name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                );
            },
            /*pollFunc:*/ nativeModule().{{ callable.ffi_rust_future_poll(ci) }},
            /*completeFunc:*/ nativeModule().{{ callable.ffi_rust_future_complete(ci) }},
            /*freeFunc:*/ nativeModule().{{ callable.ffi_rust_future_free(ci) }},
            {%- match callable.return_type() %}
            {%- when Some(return_type) %}
            /*liftFunc:*/ {{ return_type|lift_fn }},
            {%- when None %}
            liftFunc: (_v) => {},
            {%- endmatch %}
            {%- match callable.throws_type() %}
            {%- when Some with (e) %}
            /*errorHandler:*/ {{ e|ffi_error_converter_name }}.lift
            {%- else %}
            /*errorHandler:*/ undefined,
            {% endmatch %}
        })
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

{%- macro await(func) %}
{%- if func.is_async() %}await {% endif %}
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
