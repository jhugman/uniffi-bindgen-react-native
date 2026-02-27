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
        {{- self.import_infra("UniffiRustCaller", "rust-call") }}
        uniffiCaller.rustCallWithError(
            /*liftError:*/ {{ e|lift_error_fn(self) }},
            /*caller:*/ (callStatus) => {
    {%- else -%}
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => {
    {%- endmatch %}
            {%- if func.return_type().is_some() %}
                return
            {%- endif %} {% call native_method_handle(func.ffi_func().name()) %}(
                {%- if func.self_type().is_some() %}{{ obj_factory }}.clonePointer(this), {% endif %}
                {%- call arg_list_lowered(func) %}
                callStatus);
            },
            /*liftString:*/ FfiConverterString.lift,
    )
{%- endmacro -%}

// Like to_ffi_method_call, but for value-type receivers (enums, records) which
// lower their "self" via an FfiConverter rather than a pointer clone.
{%- macro to_ffi_value_call(ffi_converter, self_value, func) -%}
    {%- match func.throws_type() -%}
    {%- when Some with (e) -%}
        {{- self.import_infra("UniffiRustCaller", "rust-call") }}
        uniffiCaller.rustCallWithError(
            /*liftError:*/ {{ e|lift_error_fn(self) }},
            /*caller:*/ (callStatus) => {
    {%- else -%}
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => {
    {%- endmatch %}
            {%- if func.return_type().is_some() %}
                return
            {%- endif %} {% call native_method_handle(func.ffi_func().name()) %}(
                {{ ffi_converter }}.lower({{ self_value }}),
                {%- call arg_list_lowered(func) %}
                callStatus);
            },
            /*liftString:*/ FfiConverterString.lift,
    )
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
{{ prefix }}{% call async_kw(callable) %}{{ func_decl }} {{ callable.name()|fn_name }}(
    {%- call arg_list_decl(callable) -%}): {# space #}

    {%- call return_type(callable) %}
    {%- call throws_kw(callable) %} {
    {%- call call_body(obj_factory, callable) %}
    }
{%- endmacro %}

{%- macro return_type(callable) %}
    {%- if callable.is_async() %}Promise<{% call raw_return_type(callable) %}>
    {%- else %}
    {%- call raw_return_type(callable) %}
    {%- endif %}
{%- endmacro %}

{%- macro raw_return_type(callable) %}
    {%- match callable.return_type() %}
    {%-  when Some with (return_type) %}{{ return_type|type_name(self) }}
    {%-  when None %}void
    {%- endmatch %}
{%- endmacro %}

// primary ctor - no name, no return-type.
{%- macro ctor_decl(obj_factory, callable, indent) %}
{%- call docstring(callable, indent) %}
    constructor(
    {%- call arg_list_decl(callable) -%}) {%- call throws_kw(callable) %} {
        super();
        const pointer =
            {% call to_ffi_method_call(obj_factory, callable) %};
        this[pointerLiteralSymbol] = pointer;
        this[destructorGuardSymbol] = {{ obj_factory }}.bless(pointer);
    }
{%- endmacro %}

{%- macro call_body(obj_factory, callable) %}
{%- if callable.is_async() %}
{%-   if flavor.supports_rust_backtrace() %}
    return {# space #}{%- call call_async(obj_factory, callable) %};
{%-   else %}
    const __stack = uniffiIsDebug ? new Error().stack : undefined;
    try {
        return {# space #}{%- call call_async(obj_factory, callable) %};
    } catch (__error: any) {
        if (uniffiIsDebug && __error instanceof Error) {
            __error.stack = __stack;
        }
        throw __error;
    }
{%-  endif %}
{%- else %}
{%-     match callable.return_type() -%}
{%-         when Some with (return_type) %}
    return {{ return_type|ffi_converter_name(self) }}.lift({% call to_ffi_method_call(obj_factory, callable) %});
{%-         when None %}
{%-             call to_ffi_method_call(obj_factory, callable) %};
{%-     endmatch %}
{%- endif %}

{%- endmacro %}

// Like call_body, but for value-type receivers (enums, records).
// Trait methods are never async so no async handling needed.
{%- macro call_body_value(ffi_converter, self_value, callable) %}
{%- match callable.return_type() -%}
{%-     when Some with (return_type) %}
    return {{ return_type|ffi_converter_name(self) }}.lift({% call to_ffi_value_call(ffi_converter, self_value, callable) %});
{%-     when None %}
{%-         call to_ffi_value_call(ffi_converter, self_value, callable) %};
{%- endmatch %}
{%- endmacro %}

// Generates uniffi trait instance methods for a value-type receiver (e.g. enum variant classes).
// `self_lower` is the expression used to lower the receiver (e.g. "this as unknown as TypeName").
// `type_name` is used for the parameter type of binary methods (equals, compareTo).
{%- macro uniffi_trait_methods_instance(tm, ffi_converter, self_lower, type_name) %}
{%- if let Some(fmt) = tm.display_fmt %}
    toString(): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, self_lower, fmt) %}
    }
{%- endif %}
{%- if let Some(fmt) = tm.debug_fmt %}
    toDebugString(): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, self_lower, fmt) %}
    }
{%- if tm.display_fmt.is_none() %}
    toString(): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, self_lower, fmt) %}
    }
{%- endif %}
{%- endif %}
{%- if let Some(eq) = tm.eq_eq %}
    equals(other: {{ type_name }}): {% call return_type(eq) %} {
        {% call call_body_value(ffi_converter, self_lower, eq) %}
    }
{%- endif %}
{%- if let Some(hash) = tm.hash_hash %}
    hashCode(): {% call return_type(hash) %} {
        {% call call_body_value(ffi_converter, self_lower, hash) %}
    }
{%- endif %}
{%- if let Some(cmp) = tm.ord_cmp %}
    compareTo(other: {{ type_name }}): {% call return_type(cmp) %} {
        {% call call_body_value(ffi_converter, self_lower, cmp) %}
    }
{%- endif %}
{%- endmacro %}

// Generates uniffi trait methods for a value-type receiver (e.g. records, flat enum namespaces).
// `method_prefix` prepends each method declaration; must begin with 4 spaces (e.g. "    " or "    export function ").
// `method_suffix` appends after each closing brace (e.g. "," for object literal, "" for namespace).
// Note: the closing brace is hardcoded at 4-space indent, so method_prefix must start with 4 spaces.
{%- macro uniffi_trait_methods_value_receiver(tm, ffi_converter, type_name, method_prefix, method_suffix) %}
{%- if let Some(fmt) = tm.display_fmt %}
{{ method_prefix }}toString(value: {{ type_name }}): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, "value", fmt) %}
    }{{ method_suffix }}
{%- endif %}
{%- if let Some(fmt) = tm.debug_fmt %}
{{ method_prefix }}toDebugString(value: {{ type_name }}): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, "value", fmt) %}
    }{{ method_suffix }}
{%- if tm.display_fmt.is_none() %}
{{ method_prefix }}toString(value: {{ type_name }}): {% call return_type(fmt) %} {
        {% call call_body_value(ffi_converter, "value", fmt) %}
    }{{ method_suffix }}
{%- endif %}
{%- endif %}
{%- if let Some(eq) = tm.eq_eq %}
{{ method_prefix }}equals(value: {{ type_name }}, {% call arg_list_decl(eq) %}): {% call return_type(eq) %} {
        {% call call_body_value(ffi_converter, "value", eq) %}
    }{{ method_suffix }}
{%- endif %}
{%- if let Some(hash) = tm.hash_hash %}
{{ method_prefix }}hashCode(value: {{ type_name }}): {% call return_type(hash) %} {
        {% call call_body_value(ffi_converter, "value", hash) %}
    }{{ method_suffix }}
{%- endif %}
{%- if let Some(cmp) = tm.ord_cmp %}
{{ method_prefix }}compareTo(value: {{ type_name }}, {% call arg_list_decl(cmp) %}): {% call return_type(cmp) %} {
        {% call call_body_value(ffi_converter, "value", cmp) %}
    }{{ method_suffix }}
{%- endif %}
{%- endmacro %}

{%- macro call_async(obj_factory, callable) -%}
{{- self.import_infra("uniffiRustCallAsync", "async-rust-call") -}}
        await uniffiRustCallAsync(
            /*rustCaller:*/ uniffiCaller,
            /*rustFutureFunc:*/ () => {
                return {% call native_method_handle(callable.ffi_func().name()) %}(
                    {%- if callable.self_type().is_some() %}
                    {{ obj_factory }}.clonePointer(this){% if !callable.arguments().is_empty() %},{% endif %}
                    {% endif %}
                    {%- for arg in callable.arguments() -%}
                    {{ arg|ffi_converter_name(self) }}.lower({{ arg.name()|var_name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                );
            },
            /*pollFunc:*/ {% call native_method_handle_poll(callable.ffi_rust_future_poll(ci)) %},
            /*cancelFunc:*/ {% call native_method_handle_cancel(callable.ffi_rust_future_cancel(ci)) %},
            /*completeFunc:*/ {% call native_method_handle_complete(callable.ffi_rust_future_complete(ci)) %},
            /*freeFunc:*/ {% call native_method_handle_free(callable.ffi_rust_future_free(ci)) %},
            {%- match callable.return_type() %}
            {%- when Some(return_type) %}
            /*liftFunc:*/ {{ return_type|lift_fn(self) }},
            {%- when None %}
            /*liftFunc:*/ (_v) => {},
            {%- endmatch %}
            /*liftString:*/ FfiConverterString.lift,
            /*asyncOpts:*/ asyncOpts_,
            {%- match callable.throws_type() %}
            {%- when Some with (e) %}
            /*errorHandler:*/ {{ e|lift_error_fn(self) }}
            {%- else %}
            {% endmatch %}
        )
{%- endmacro %}

{%- macro arg_list_lowered(func) %}
    {%- for arg in func.arguments() %}
        {{ arg|ffi_converter_name(self) }}.lower({{ arg.name()|var_name }}),
    {%- endfor %}
{%- endmacro -%}

{#-
// Arglist as used in ts declarations of methods, functions and constructors.
// Note the var_name and type_name filters.
-#}

{% macro arg_list_decl(func) %}
    {%- for arg in func.arguments() -%}
        {{ arg.name()|var_name }}: {{ arg|type_name(self) -}}
        {%- match arg.default_value() %}
        {%- when Some with(literal) %} = {{ literal|render_literal(arg, ci) }}
        {%- else %}
        {%- endmatch %}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
    {%- if func.is_async() %}
    {%-   if !func.arguments().is_empty() %}, {% endif -%}
    asyncOpts_?: { signal: AbortSignal }
    {%- endif %}

{%- endmacro %}

{#-
// Field lists as used in ts declarations of Records and Enums.
// Note the var_name and type_name filters.
-#}
{%- macro field_list_decl(item, has_nameless_fields) %}
    {%- for field in item.fields() -%}
    {%-   call docstring(field, 8) %}
    {%-   if has_nameless_fields -%}
    v{{ loop.index0 }}: {# space #}
    {{-     field|type_name(self) }}
    {%-   else %}
    {{-     field.name()|var_name }}: {{ field|type_name(self) -}}
    {%-     match field.default_value() %}
    {%-       when Some with(literal) %} = {{ literal|render_literal(field, ci) }}
    {%-       else %}
    {%-     endmatch -%}
    {%-   endif %}
    {%-   if !loop.last %}, {% endif %}
    {%- endfor %}
{%- endmacro %}

{%- macro field_list(item, has_nameless_fields) %}
{%- for field in item.fields() %}
{%-   if has_nameless_fields -%}
        v{{ loop.index0 }}
{%-   else %}
{{-     field.name()|var_name }}
{%-   endif %}
{%-   if !loop.last %}, {% endif %}
{%- endfor %}
{%- endmacro %}


{% macro field_name(inner, field, field_num) %}
{%- if field.name().is_empty() -%}
{{- inner }}[{{ field_num }}]
{%- else -%}
{{- inner }}.{{ field.name()|var_name }}
{%- endif -%}
{%- endmacro %}

{#-
// This macros is almost identical to `arg_list_decl`,
// but is for interface methods, which do not allow
// default values for arguments.
#}
{% macro arg_list_protocol(func) %}
    {%- for arg in func.arguments() -%}
        {{ arg.name()|var_name }}: {{ arg|type_name(self) -}}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
    {%- if func.is_async() %}
    {%-   if !func.arguments().is_empty() %}, {% endif -%}
    asyncOpts_?: { signal: AbortSignal }
    {%- endif %}
{%- endmacro %}

{%- macro async_kw(func) %}
{%- if func.is_async() %}async {% endif %}
{%- endmacro -%}

{%- macro await_kw(func) %}
{%- if func.is_async() %}await {% endif %}
{%- endmacro -%}

{%- macro throws_kw(func) %}
{%- if func.throws() %} /*throws*/{% endif %}
{%- endmacro -%}

{%- macro try_kw(func) %}
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

{%- macro type_omit_instanceof(type_name, decl_type_name) %}
export type {{ type_name }} = InstanceType<
    typeof {{ decl_type_name }}[keyof Omit<typeof {{ decl_type_name }}, 'instanceOf'>]
>;
{%- endmacro %}

{%- macro fn_handle(func) %}
{%- call fn_handle_with_name(func.name()) %}
{%- endmacro %}

{%- macro fn_handle_with_name(name) %}nativeModule().
{%- call ffi_func_name(name) %}
{%- endmacro %}

{%- macro ffi_func_name(name) %}ubrn_{{ name }}
{%- endmacro %}


{#
// Verbose logging.
#}

{#
// Most function calls use the `native_method_handle` macro, which if the loglevel in uniffi.toml
// is not set to `verbose`, then a call into Rust is:
// ```ts
// nativeModule().uniffi_uniffi_futures_fn_func_use_shared_resource
// ```
// If set to verbose, then it is:
// ```ts
// (() => {
//   console.debug("-- uniffi_uniffi_futures_fn_func_use_shared_resource");
//   return nativeModule().uniffi_uniffi_futures_fn_func_use_shared_resource;
// })()
// ```
// When this IIFE is called, it is usually immediately before the function is being inovked:
// when not verbose:
// ```ts
// nativeModule().uniffi_uniffi_futures_fn_func_use_shared_resource(theArgument)
// ```
// and when verbose:
// ```ts
// (() => {
//   console.debug("-- uniffi_uniffi_futures_fn_func_use_shared_resource");
//   return nativeModule().uniffi_uniffi_futures_fn_func_use_shared_resource;
// })()(theArgument)
// ```
#}

{%- macro native_method_handle(method_name) %}
{%- if config.is_verbose() -%}
(() => {
    {% call log_call(method_name) %}
    return {% call fn_handle_with_name(method_name) %};
})()
{%- else -%}
{% call fn_handle_with_name(method_name) %}
{%- endif %}
{%- endmacro %}


{#
// uniffiRustCallAsync calls take several function handles as arguments, then call
// into Rust in several stages.
//
// There now follow several macros which generate anonymous functions which log
// the method call, and then forward the arguments on to Rust.
//
// It is these function literals that are passed to uniffiRustCallAsync instead of the bare
// function handles.
#}

{%- macro native_method_handle_poll(method_name) %}
{%- if config.is_verbose() -%}
{{- self.import_infra_type("UniffiHandle", "handle-map") }}
(rustFuture: bigint, cb: UniffiRustFutureContinuationCallback, handle: UniffiHandle): void => {
    {% call log_message("   poll    : ", method_name, "") %}
    return {% call fn_handle_with_name(method_name) %}(rustFuture, cb, handle);
}
{%- else -%}
{% call fn_handle_with_name(method_name) %}
{%- endif %}
{%- endmacro %}

{%- macro native_method_handle_cancel(method_name) %}
{%- if config.is_verbose() -%}
(rustFuture: bigint): void => {
    {% call log_message("   cancel  : ", method_name, "") %}
    return {% call fn_handle_with_name(method_name) %}(rustFuture);
}
{%- else -%}
{% call fn_handle_with_name(method_name) %}
{%- endif %}
{%- endmacro %}

{%- macro native_method_handle_complete(method_name) %}
{%- if config.is_verbose() -%}
{{- self.import_infra_type("UniffiRustCallStatus", "rust-call")}}
(rustFuture: bigint, status: UniffiRustCallStatus) => {
    {% call log_message("   complete: ", method_name, "") %}
    return {% call fn_handle_with_name(method_name) %}(rustFuture, status);
}
{%- else -%}
{% call fn_handle_with_name(method_name) %}
{%- endif %}
{%- endmacro %}

{%- macro native_method_handle_free(method_name) %}
{%- if config.is_verbose() -%}
(rustFuture: bigint) => {
    {% call log_message("   free    : ", method_name, "") %}
    return {% call fn_handle_with_name(method_name) %}(rustFuture);
}
{%- else -%}
{% call fn_handle_with_name(method_name) %}
{%- endif %}
{%- endmacro %}

{%- macro log_call(method_name) %}
{%- call log_message("", method_name, "") %}
{%- endmacro %}

{%- macro log_message(prefix, middle, suffix) %}
{%- if config.is_verbose() %}
{%- call set_up_console_log %}
console.debug(`-- {{ prefix }}{{ middle }}{{ suffix }}`);
{%- endif %}
{%- endmacro %}

{%- macro set_up_console_log() %}
{%- match config.console_import %}
{%- when Some(module) %}
{{- self.import_custom("console", module) }}
{%- else %}
{%- endmatch %}
{%- endmacro %}
