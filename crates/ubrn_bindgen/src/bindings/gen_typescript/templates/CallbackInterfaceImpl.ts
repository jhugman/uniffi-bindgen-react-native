{{- self.import_infra_type("UniffiHandle", "handle-map") }}
{{- self.import_infra_type("UniffiReferenceHolder", "callbacks") }}
{{- self.import_infra("uniffiCreateCallStatus", "rust-call")}}
{{- self.import_infra_type("UniffiRustCallStatus", "rust-call")}}
{{- self.import_infra("RustBuffer", "ffi-types")}}

{%- let vtable_methods = cbi.vtable_methods() %}
{%- let trait_impl = format!("uniffiCallbackInterface{}", name) %}

// Put the implementation in a struct so we don't pollute the top-level namespace
const {{ trait_impl }}: { vtable: {{ vtable|ffi_type_name }}; register: () => void; } = {
    // Create the VTable using a series of closures.
    // ts automatically converts these into C callback functions.
    vtable: {
        {%- for (ffi_callback, meth) in vtable_methods %}
        {{ meth.name()|fn_name }}: (
            {%- for arg in ffi_callback.arguments() %}
            {{ arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name }}{% if !loop.last || ffi_callback.has_rust_call_status_arg() %},{% endif %}
            {%- endfor -%}
            {%- if ffi_callback.has_rust_call_status_arg() %}
            uniffiCallStatus: UniffiRustCallStatus
            {%- endif %}
        ) => {
            const uniffiMakeCall = {# space #}
            {%- if meth.is_async() %}
            async (signal: AbortSignal)
            {%- else %}
            ()
            {%- endif %}
            : {% call ts::return_type(meth) %} => {
                const jsCallback = {{ ffi_converter_name }}.lift(uniffiHandle);
                return {% call ts::await(meth) %}jsCallback.{{ meth.name()|fn_name }}(
                    {%- for arg in meth.arguments() %}
                    {{ arg|ffi_converter_name(self) }}.lift({{ arg.name()|var_name }}){% if !loop.last %}, {% endif %}
                    {%- endfor %}
                    {%- if meth.is_async() -%}
                    {%-   if !meth.arguments().is_empty() %}, {% endif -%}
                    { signal }
                    {%- endif %}
                )
            }
            {%- if !meth.is_async() %}

            {% match meth.return_type() %}
            {%- when Some(t) %}
            const uniffiWriteReturn = (obj: any) => { uniffiOutReturn.pointee = {{ t|ffi_converter_name(self) }}.lower(obj) };
            {%- when None %}
            const uniffiWriteReturn = (obj: any) => {};
            {%- endmatch %}

            {%- match meth.throws_type() %}
            {%- when None %}
            {{- self.import_infra("uniffiTraitInterfaceCall", "callbacks") }}
            uniffiTraitInterfaceCall(
                /*callStatus:*/ uniffiCallStatus,
                /*makeCall:*/ uniffiMakeCall,
                /*writeReturn:*/ uniffiWriteReturn,
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- when Some(error_type) %}
            {{- self.import_infra("uniffiTraitInterfaceCallWithError", "callbacks") }}
            uniffiTraitInterfaceCallWithError(
                /*callStatus:*/ uniffiCallStatus,
                /*makeCall:*/ uniffiMakeCall,
                /*writeReturn:*/ uniffiWriteReturn,
                /*isErrorType:*/ {{ error_type|decl_type_name(self) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_error_fn(self) }},
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- endmatch %}
            {%- else %} {# // is_async = true #}

            const uniffiHandleSuccess = (returnValue: {% call ts::raw_return_type(meth) %}) => {
                uniffiFutureCallback(
                    uniffiCallbackData,
                    /* {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }} */{
                        {%- match meth.return_type() %}
                        {%- when Some(return_type) %}
                        returnValue: {{ return_type|ffi_converter_name(self) }}.lower(returnValue),
                        {%- when None %}
                        {%- endmatch %}
                        callStatus: uniffiCreateCallStatus()
                    }
                );
            };
            const uniffiHandleError = (code: number, errorBuf: ArrayBuffer) => {
                uniffiFutureCallback(
                    uniffiCallbackData,
                    /* {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }} */{
                        {%- match meth.return_type().map(FfiType::from) %}
                        {%- when Some(return_type) %}
                        returnValue: {{ return_type|ffi_default_value }},
                        {%- when None %}
                        {%- endmatch %}
                        callStatus: { code, errorBuf }
                    }
                );
            };

            {%- match meth.throws_type() %}
            {%- when None %}
            {{- self.import_infra("uniffiTraitInterfaceCallAsync", "async-callbacks") }}
            let uniffiForeignFuture = uniffiTraitInterfaceCallAsync(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- when Some(error_type) %}
            {{- self.import_infra("uniffiTraitInterfaceCallAsyncWithError", "async-callbacks") }}
            let uniffiForeignFuture = uniffiTraitInterfaceCallAsyncWithError(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*isErrorType:*/ {{ error_type|decl_type_name(self) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_error_fn(self) }},
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- endmatch %}
            uniffiOutReturn.pointee = uniffiForeignFuture
            {%- endif %}
        },
        {%- endfor %}
        uniffiFree: (uniffiHandle: UniffiHandle): void => {
            // {{ name }}: this will throw a stale handle error if the handle isn't found.
            {{ ffi_converter_name }}.drop(uniffiHandle);
        }
    },
    register: () => {
        nativeModule().{{ cbi.ffi_init_callback().name() }}(
            {{ trait_impl }}.vtable
        );
    },
};
