{{- self.import_infra_type("UniffiHandle", "handle-map") }}
{{- self.import_infra_type("UniffiReferenceHolder", "callbacks") }}
{{- self.import_infra_type("UniffiByteArray", "ffi-types")}}
{{- self.import_infra("UniffiRustCaller", "rust-call")}}
{{- self.import_infra("UniffiResult", "result")}}
{{- self.import_infra_type("UniffiRustCallStatus", "rust-call")}}

{%- let vtable_methods = cbi.vtable_methods() %}
{%- let trait_impl = format!("uniffiCallbackInterface{}", name) %}

// Put the implementation in a struct so we don't pollute the top-level namespace
const {{ trait_impl }}: { vtable: {{ vtable|ffi_type_name }}; register: () => void; } = {
    // Create the VTable using a series of closures.
    // ts automatically converts these into C callback functions.
    vtable: {
        {%- for (ffi_callback, meth) in vtable_methods %}
        {{ meth.name()|fn_name }}: (
            {%- for arg in ffi_callback.arguments_no_return() %}
            {{ arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name }}{% if !loop.last || ffi_callback.has_rust_call_status_arg() %},{% endif %}
            {%- endfor -%}
        ) => {
            const uniffiMakeCall = {# space #}
            {%- if meth.is_async() %}
            async (signal: AbortSignal)
            {%- else %}
            ()
            {%- endif %}
            : {% call ts::return_type(meth) %} => {
                const jsCallback = {{ ffi_converter_name }}.lift(uniffiHandle);
                return {% call ts::await_kw(meth) %}jsCallback.{{ meth.name()|fn_name }}(
                    {%- for arg in meth.arguments() %}
                    {{ arg|ffi_converter_name(self) }}.lift({{ arg.name()|var_name }}){% if !loop.last %}, {% endif %}
                    {%- endfor %}
                    {%- if meth.is_async() -%}
                    {%-   if !meth.arguments().is_empty() %}, {% endif -%}
                    { signal }
                    {%- endif %}
                )
            };
            {%- if !meth.is_async() %}
            {#- // Synchronous callback method #}
            {%- match meth.return_type() %}
            {%- when Some(t) %}
            const uniffiResult = UniffiResult.ready<{{ t|ffi_type_name_from_type(self) }}>();
            const uniffiHandleSuccess = (obj: any) => {
                UniffiResult.writeSuccess(uniffiResult, {{ t|ffi_converter_name(self) }}.lower(obj));
            };
            {%- when None %}
            const uniffiResult = UniffiResult.ready<void>();
            const uniffiHandleSuccess = (obj: any) => {};
            {%- endmatch %}
            const uniffiHandleError = (code: number, errBuf: UniffiByteArray) => {
                UniffiResult.writeError(uniffiResult, code, errBuf);
            };

            {%- match meth.throws_type() %}
            {%- when None %}
            {{- self.import_infra("uniffiTraitInterfaceCall", "callbacks") }}
            uniffiTraitInterfaceCall(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- when Some(error_type) %}
            {{- self.import_infra("uniffiTraitInterfaceCallWithError", "callbacks") }}
            uniffiTraitInterfaceCallWithError(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*isErrorType:*/ {{ error_type|decl_type_name(self) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_error_fn(self) }},
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- endmatch %}
            return uniffiResult;
            {%- else %} {#- // is_async = true #}
            {#- // Asynchronous callback method #}
            const uniffiHandleSuccess = (returnValue: {% call ts::raw_return_type(meth) %}) => {
                uniffiFutureCallback.call(
                    uniffiFutureCallback,
                    uniffiCallbackData,
                    /* {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }} */{
                        {%- match meth.return_type() %}
                        {%- when Some(return_type) %}
                        returnValue: {{ return_type|ffi_converter_name(self) }}.lower(returnValue),
                        {%- when None %}
                        {%- endmatch %}
                        callStatus: uniffiCaller.createCallStatus()
                    }
                );
            };
            const uniffiHandleError = (code: number, errorBuf: UniffiByteArray) => {
                uniffiFutureCallback.call(
                    uniffiFutureCallback,
                    uniffiCallbackData,
                    /* {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }} */{
                        {%- match meth.return_type().map(FfiType::from) %}
                        {%- when Some(return_type) %}
                        returnValue: {{ return_type|ffi_default_value }},
                        {%- when None %}
                        {%- endmatch %}
                        // TODO create callstatus with error.
                        callStatus: uniffiCaller.createErrorStatus(code, errorBuf),
                    }
                );
            };

            {%- match meth.throws_type() %}
            {%- when None %}
            {{- self.import_infra("uniffiTraitInterfaceCallAsync", "async-callbacks") }}
            const uniffiForeignFuture = uniffiTraitInterfaceCallAsync(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- when Some(error_type) %}
            {{- self.import_infra("uniffiTraitInterfaceCallAsyncWithError", "async-callbacks") }}
            const uniffiForeignFuture = uniffiTraitInterfaceCallAsyncWithError(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*isErrorType:*/ {{ error_type|decl_type_name(self) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_error_fn(self) }},
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- endmatch %}
            return uniffiForeignFuture;
            {%- endif %}
        },
        {%- endfor %}
        uniffiFree: (uniffiHandle: UniffiHandle): void => {
            // {{ name }}: this will throw a stale handle error if the handle isn't found.
            {{ ffi_converter_name }}.drop(uniffiHandle);
        }
    },
    register: () => {
        {% call ts::fn_handle(cbi.ffi_init_callback()) %}(
            {{ trait_impl }}.vtable
        );
    },
};
