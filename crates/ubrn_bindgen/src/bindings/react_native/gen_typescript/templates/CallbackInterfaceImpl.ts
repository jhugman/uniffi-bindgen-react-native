{%- if self.include_once_check("CallbackInterfaceRuntime.ts") %}{%- include "CallbackInterfaceRuntime.ts" %}{%- endif %}
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
            {%- call ts::async(meth) -%}
            (): {% call ts::return_type(meth) %} => {
                const uniffiObj = {{ ffi_converter_name }}.lift(uniffiHandle);
                if (uniffiObj === undefined) {
                    throw new UniffiInternalError.UnexpectedStaleHandle()
                }
                return {% call ts::await(meth) %}uniffiObj.{{ meth.name()|fn_name }}(
                    {%- for arg in meth.arguments() %}
                    {{ arg|ffi_converter_name }}.lift({{ arg.name()|var_name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                )
            }
            {%- if !meth.is_async() %}

            {% match meth.return_type() %}
            {%- when Some(t) %}
            const uniffiWriteReturn = (obj: any) => { uniffiOutReturn.pointee = {{ t|lower_fn }}(obj) };
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
                /*isErrorType:*/ {{ error_type|decl_type_name(ci) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_fn }},
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- endmatch %}
            {%- else %}

            const uniffiHandleSuccess = (returnValue: {% call ts::raw_return_type(meth) %}) => {
                uniffiFutureCallback(
                    uniffiCallbackData,
                    /* {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }} */{
                        {%- match meth.return_type() %}
                        {%- when Some(return_type) %}
                        returnValue: {{ return_type|ffi_converter_name }}.lower(returnValue),
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
                /*isErrorType:*/ {{ error_type|decl_type_name(ci) }}.instanceOf,
                /*lowerError:*/ {{ error_type|lower_fn }},
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
