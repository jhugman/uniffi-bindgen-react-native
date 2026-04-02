{#- Shared macros for rendering sync and async call bodies from TsCallable IR nodes.

    These macros expect `module: &TsApiModule` to be in the template context.
-#}

{#- The main ffi function handle, optionally wrapped in a verbose IIFE. -#}
{%- macro native_method_handle(ffi_name) %}
{%- if module.is_verbose -%}
(() => {
    {% call log_call(ffi_name) %}
    return nativeModule().{{ ffi_name }};
})()
{%- else -%}
nativeModule().{{ ffi_name }}
{%- endif %}
{%- endmacro %}

{#- Log a call in verbose mode. -#}
{%- macro log_call(ffi_name) %}
{%- if module.is_verbose %}
{%- if let Some(console_mod) = module.console_import %}
// import console from "{{ console_mod }}";
{%- endif %}
console.debug(`-- {{ ffi_name }}`);
{%- endif %}
{%- endmacro %}

{#- Return type rendering. -#}
{%- macro return_type(callable) %}
    {%- if callable.is_async() %}Promise<{% call raw_return_type(callable) %}>
    {%- else %}
    {%- call raw_return_type(callable) %}
    {%- endif %}
{%- endmacro %}

{%- macro raw_return_type(callable) %}
    {%- match callable.return_type %}
    {%-  when Some with (rt) %}{{ rt.ts_type }}
    {%-  when None %}void
    {%- endmatch %}
{%- endmacro %}

{#- Throws keyword. -#}
{%- macro throws_kw(callable) %}
{%- if callable.is_throwing() %} /*throws*/{% endif %}
{%- endmacro %}

{#- Argument list declaration (for function/method signatures). -#}
{%- macro arg_list_decl(callable) %}
    {%- for arg in callable.arguments -%}
        {{ arg.name }}: {{ arg.ts_type -}}
        {%- if let Some(dv) = arg.default_value %} = {{ dv }}{% endif %}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
    {%- if callable.is_async() %}
    {%-   if !callable.arguments.is_empty() %}, {% endif -%}
    asyncOpts_?: { signal: AbortSignal }
    {%- endif %}
{%- endmacro %}

{#- Argument list for protocol/interface declarations (no default values). -#}
{%- macro arg_list_protocol(callable) %}
    {%- for arg in callable.arguments -%}
        {{ arg.name }}: {{ arg.ts_type -}}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor %}
    {%- if callable.is_async() %}
    {%-   if !callable.arguments.is_empty() %}, {% endif -%}
    asyncOpts_?: { signal: AbortSignal }
    {%- endif %}
{%- endmacro %}

{#- Lowered argument list (for FFI calls). -#}
{%- macro arg_list_lowered(callable) %}
    {%- for arg in callable.arguments %}
        {{ arg.ffi_converter }}.lower({{ arg.name }}),
    {%- endfor %}
{%- endmacro -%}

{#- Sync FFI call with pointer receiver. -#}
{%- macro to_ffi_pointer_call(callable, obj_factory) -%}
    {%- match callable.throws -%}
    {%- when Some with (e) -%}
        uniffiCaller.rustCallWithError(
            /*liftError:*/ {{ e.lift_error_fn }},
            /*caller:*/ (callStatus) => {
    {%- else -%}
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => {
    {%- endmatch %}
            {%- if callable.return_type.is_some() %}
                return
            {%- endif %} {% call native_method_handle(callable.ffi_name) %}(
                {{ obj_factory }}.clonePointer(this),
                {%- call arg_list_lowered(callable) %}
                callStatus);
            },
            /*liftString:*/ FfiConverterString.lift.bind(FfiConverterString),
    )
{%- endmacro -%}

{#- Sync FFI call with no receiver (top-level function or constructor). -#}
{%- macro to_ffi_call(callable) -%}
    {%- match callable.throws -%}
    {%- when Some with (e) -%}
        uniffiCaller.rustCallWithError(
            /*liftError:*/ {{ e.lift_error_fn }},
            /*caller:*/ (callStatus) => {
    {%- else -%}
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => {
    {%- endmatch %}
            {%- if callable.return_type.is_some() %}
                return
            {%- endif %} {% call native_method_handle(callable.ffi_name) %}(
                {%- call arg_list_lowered(callable) %}
                callStatus);
            },
            /*liftString:*/ FfiConverterString.lift.bind(FfiConverterString),
    )
{%- endmacro -%}

{#- Sync FFI call with value receiver (enum/record methods). -#}
{%- macro to_ffi_value_call(callable) -%}
    {%- match callable.value_receiver_ffi_converter() -%}
    {%- when Some with (ffi_converter) -%}
    {%- match callable.throws -%}
    {%- when Some with (e) -%}
        uniffiCaller.rustCallWithError(
            /*liftError:*/ {{ e.lift_error_fn }},
            /*caller:*/ (callStatus) => {
    {%- else -%}
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => {
    {%- endmatch %}
            {%- if callable.return_type.is_some() %}
                return
            {%- endif %} {% call native_method_handle(callable.ffi_name) %}(
                {{ ffi_converter }}.lower(self_),
                {%- call arg_list_lowered(callable) %}
                callStatus);
            },
            /*liftString:*/ FfiConverterString.lift.bind(FfiConverterString),
    )
    {%- else -%}
    {#- unreachable -#}
    {%- endmatch %}
{%- endmacro -%}

{#- Call body for value-receiver method: sync only (trait methods are never async). -#}
{%- macro call_body_value(callable) %}
{%- match callable.return_type -%}
{%-     when Some with (return_type) %}
    return {{ return_type.ffi_converter }}.lift({% call to_ffi_value_call(callable) %});
{%-     when None %}
{%-         call to_ffi_value_call(callable) %};
{%- endmatch %}
{%- endmacro %}

{#- Call body for method (pointer receiver): sync or async. -#}
{%- macro call_body_method(callable, obj_factory) %}
{%- if callable.is_async() %}
{%-   call call_body_async(callable, obj_factory) %}
{%- else %}
{%-     match callable.return_type -%}
{%-         when Some with (return_type) %}
    return {{ return_type.ffi_converter }}.lift({% call to_ffi_pointer_call(callable, obj_factory) %});
{%-         when None %}
{%-             call to_ffi_pointer_call(callable, obj_factory) %};
{%-     endmatch %}
{%- endif %}
{%- endmacro %}

{#- Call body for function (no receiver): sync or async. -#}
{%- macro call_body_function(callable) %}
{%- if callable.is_async() %}
{%-   call call_body_async(callable, "unreachable") %}
{%- else %}
{%-     match callable.return_type -%}
{%-         when Some with (return_type) %}
    return {{ return_type.ffi_converter }}.lift({% call to_ffi_call(callable) %});
{%-         when None %}
{%-             call to_ffi_call(callable) %};
{%-     endmatch %}
{%- endif %}
{%- endmacro %}

{#- Async call body: wraps uniffiRustCallAsync with optional stack trace capture. -#}
{%- macro call_body_async(callable, obj_factory) %}
{%- if module.supports_rust_backtrace %}
    return {# space #}{%- call to_ffi_async_call(callable, obj_factory) %};
{%- else %}
    const __stack = uniffiIsDebug ? new Error().stack : undefined;
    try {
        return {# space #}{%- call to_ffi_async_call(callable, obj_factory) %};
    } catch (__error: any) {
        if (uniffiIsDebug && __error instanceof Error) {
            __error.stack = __stack;
        }
        throw __error;
    }
{%- endif %}
{%- endmacro %}

{#- Generates the uniffiRustCallAsync(...) invocation. -#}
{%- macro to_ffi_async_call(callable, obj_factory) %}
{%- match callable.ffi_async %}
{%- when Some with (ffi_async) -%}
        await uniffiRustCallAsync(
            /*rustCaller:*/ uniffiCaller,
            /*rustFutureFunc:*/ () => {
                return {% call native_method_handle(callable.ffi_name) %}(
                    {%- if callable.receiver.is_some() %}
                    {{ obj_factory }}.clonePointer(this){% if !callable.arguments.is_empty() %},{% endif %}
                    {%- endif %}
                    {%- for arg in callable.arguments -%}
                    {{ arg.ffi_converter }}.lower({{ arg.name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                );
            },
            /*pollFunc:*/ {% call native_method_handle_poll(ffi_async.poll) %},
            /*cancelFunc:*/ {% call native_method_handle_cancel(ffi_async.cancel) %},
            /*completeFunc:*/ {% call native_method_handle_complete(ffi_async.complete) %},
            /*freeFunc:*/ {% call native_method_handle_free(ffi_async.free) %},
            {%- match callable.return_type %}
            {%- when Some(return_type) %}
            /*liftFunc:*/ {{ return_type.ffi_converter }}.lift.bind({{ return_type.ffi_converter }}),
            {%- when None %}
            /*liftFunc:*/ (_v) => {},
            {%- endmatch %}
            /*liftString:*/ FfiConverterString.lift.bind(FfiConverterString),
            /*asyncOpts:*/ asyncOpts_,
            {%- match callable.throws %}
            {%- when Some with (e) %}
            /*errorHandler:*/ {{ e.lift_error_fn }}
            {%- else %}
            {% endmatch %}
        )
{%- when None %}
{#- unreachable: caller guards with is_async() -#}
{%- endmatch %}
{%- endmacro %}

{#- Verbose-mode poll handle wrapper. -#}
{%- macro native_method_handle_poll(method_name) %}
{%- if module.is_verbose -%}
(rustFuture: bigint, cb: UniffiRustFutureContinuationCallback, handle: UniffiHandle): void => {
    {% call log_message("   poll    : ", method_name, "") %}
    return nativeModule().{{ method_name }}(rustFuture, cb, handle);
}
{%- else -%}
nativeModule().{{ method_name }}
{%- endif %}
{%- endmacro %}

{#- Verbose-mode cancel handle wrapper. -#}
{%- macro native_method_handle_cancel(method_name) %}
{%- if module.is_verbose -%}
(rustFuture: bigint): void => {
    {% call log_message("   cancel  : ", method_name, "") %}
    return nativeModule().{{ method_name }}(rustFuture);
}
{%- else -%}
nativeModule().{{ method_name }}
{%- endif %}
{%- endmacro %}

{#- Verbose-mode complete handle wrapper. -#}
{%- macro native_method_handle_complete(method_name) %}
{%- if module.is_verbose -%}
(rustFuture: bigint, status: UniffiRustCallStatus) => {
    {% call log_message("   complete: ", method_name, "") %}
    return nativeModule().{{ method_name }}(rustFuture, status);
}
{%- else -%}
nativeModule().{{ method_name }}
{%- endif %}
{%- endmacro %}

{#- Verbose-mode free handle wrapper. -#}
{%- macro native_method_handle_free(method_name) %}
{%- if module.is_verbose -%}
(rustFuture: bigint) => {
    {% call log_message("   free    : ", method_name, "") %}
    return nativeModule().{{ method_name }}(rustFuture);
}
{%- else -%}
nativeModule().{{ method_name }}
{%- endif %}
{%- endmacro %}

{#- Log a verbose message. -#}
{%- macro log_message(prefix, middle, suffix) %}
{%- if module.is_verbose %}
{%- if let Some(console_mod) = module.console_import %}
// import console from "{{ console_mod }}";
{%- endif %}
console.debug(`-- {{ prefix }}{{ middle }}{{ suffix }}`);
{%- endif %}
{%- endmacro %}

{#- Docstring rendering. -#}
{%- macro docstring(maybe_docstring) %}
{%- if let Some(ds) = maybe_docstring %}
{{ ds }}
{%- endif %}
{%- endmacro %}

{#- Variant inner type shape: Readonly<{ name: Type; ... }> or Readonly<[Type, ...]>. -#}
{%- macro variant_inner_type(variant) %}
Readonly<{%- if !variant.has_nameless_fields %}{
{%-   for field in variant.fields %}
{{-     field.name }}: {{ field.ts_type }}
{%-     if !loop.last %}; {% endif -%}
{%- endfor %}}
{%- else %}
[
{%-   for field in variant.fields %}
{{-     field.ts_type }}
{%-     if !loop.last %}, {% endif -%}
{%- endfor %}
]
{%- endif %}>
{%- endmacro %}

{#- Variant inner value shape for constructor parameters: { name: Type; ... } or unnamed positional args. -#}
{%- macro variant_fields_decl(variant) %}
{%- if !variant.has_nameless_fields %}
inner: { {%- for field in variant.fields %}{{ field.name }}: {{ field.ts_type }}{%- if !loop.last %}; {% endif %}{%- endfor %} }
{%- else %}
{%- for field in variant.fields %}v{{ loop.index0 }}: {{ field.ts_type }}{%- if !loop.last %}, {% endif %}{%- endfor %}
{%- endif %}
{%- endmacro %}

{#- Variant constructor body: freeze inner. -#}
{%- macro variant_ctor_body(variant) %}
{%- if !variant.has_nameless_fields %}
            this.inner = Object.freeze(inner);
{%- else %}
            this.inner = Object.freeze([{%- for field in variant.fields %}v{{ loop.index0 }}{%- if !loop.last %}, {% endif %}{%- endfor -%}]);
{%- endif %}
{%- endmacro %}

{#- Variant static new() forwarding args. -#}
{%- macro variant_new_args(variant) %}
{%- if !variant.has_nameless_fields %}inner{%- else %}{%- for field in variant.fields %}v{{ loop.index0 }}{%- if !loop.last %}, {% endif %}{%- endfor %}{%- endif %}
{%- endmacro %}
