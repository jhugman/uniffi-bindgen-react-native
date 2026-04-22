{%- import "CallBodyMacros.ts" as cb %}
{%- macro callback_interface_impl(vtable, ffi_converter_name, trait_impl) %}

// Put the implementation in a struct so we don't pollute the top-level namespace
const {{ trait_impl }}: { vtable: any; register: () => void; } = {
    // Create the VTable using a series of closures.
    // ts automatically converts these into C callback functions.
    vtable: {
        {%- for field in vtable.fields %}
        {%- match field.method %}
        {%- when Some with (meth) %}
        {{ field.name }}: (
            {%- for arg in field.ffi_closure_args %}
            {{ arg.name }}: {{ arg.ffi_type }}{% if !loop.last || field.has_rust_call_status_arg %},{% endif %}
            {%- endfor -%}
        ) => {
            const uniffiMakeCall = {# space #}
            {%- if meth.is_async() %}
            async (signal: AbortSignal)
            {%- else %}
            ()
            {%- endif %}
            : {% call cb::return_type(meth) %} => {
                const jsCallback = {{ ffi_converter_name }}.lift(uniffiHandle);
                return {% if meth.is_async() %}await {% endif %}jsCallback.{{ meth.name }}(
                    {%- for arg in meth.arguments %}
                    {{ arg.ffi_converter }}.lift({{ arg.name }}){% if !loop.last %}, {% endif %}
                    {%- endfor %}
                    {%- if meth.is_async() -%}
                    {%-   if !meth.arguments.is_empty() %}, {% endif -%}
                    { signal }
                    {%- endif %}
                )
            };
            {%- if !meth.is_async() %}
            {#- // Synchronous callback method #}
            {%- match meth.return_type %}
            {%- when Some(t) %}
            const uniffiResult = UniffiResult.ready<{{ t.ffi_type }}>();
            const uniffiHandleSuccess = (obj: any) => {
                UniffiResult.writeSuccess(uniffiResult, {{ t.ffi_converter }}.lower(obj));
            };
            {%- when None %}
            const uniffiResult = UniffiResult.ready<void>();
            const uniffiHandleSuccess = (obj: any) => {};
            {%- endmatch %}
            const uniffiHandleError = (code: number, errBuf: UniffiByteArray) => {
                UniffiResult.writeError(uniffiResult, code, errBuf);
            };

            {%- match meth.throws %}
            {%- when None %}
            uniffiTraitInterfaceCall(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerString:*/ FfiConverterString.lower
            )
            {%- when Some(error_type) %}
            uniffiTraitInterfaceCallWithError(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*isErrorType:*/ {{ error_type.decl_type_name }}.instanceOf,
                /*lowerError:*/ {{ error_type.lower_error_fn }},
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- endmatch %}
            return uniffiResult;
            {%- else %} {#- // is_async = true #}
            {#- // Asynchronous callback method #}
            const uniffiHandleSuccess = (returnValue: {% call cb::raw_return_type(meth) %}) => {
                uniffiFutureCallback.call(
                    uniffiFutureCallback,
                    uniffiCallbackData,
                    {%- match field.foreign_future_result %}
                    {%- when Some with (ffr) %}
                    /* {{ ffr.struct_name }} */{
                        {%- match meth.return_type %}
                        {%- when Some(return_type) %}
                        return_value: {{ return_type.ffi_converter }}.lower(returnValue),
                        {%- when None %}
                        {%- endmatch %}
                        call_status: uniffiCaller.createCallStatus()
                    }
                    {%- when None %}
                    {}
                    {%- endmatch %}
                );
            };
            const uniffiHandleError = (code: number, errorBuf: UniffiByteArray) => {
                uniffiFutureCallback.call(
                    uniffiFutureCallback,
                    uniffiCallbackData,
                    {%- match field.foreign_future_result %}
                    {%- when Some with (ffr) %}
                    /* {{ ffr.struct_name }} */{
                        {%- if !ffr.return_ffi_default_value.is_empty() %}
                        return_value: {{ ffr.return_ffi_default_value }},
                        {%- endif %}
                        // TODO create callstatus with error.
                        call_status: uniffiCaller.createErrorStatus(code, errorBuf),
                    }
                    {%- when None %}
                    {}
                    {%- endmatch %}
                );
            };

            {%- match meth.throws %}
            {%- when None %}
            const uniffiForeignFuture = uniffiTraitInterfaceCallAsync(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- when Some(error_type) %}
            const uniffiForeignFuture = uniffiTraitInterfaceCallAsyncWithError(
                /*makeCall:*/ uniffiMakeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*isErrorType:*/ {{ error_type.decl_type_name }}.instanceOf,
                /*lowerError:*/ {{ error_type.lower_error_fn }},
                /*lowerString:*/ FfiConverterString.lower
            );
            {%- endmatch %}
            return uniffiForeignFuture;
            {%- endif %}
        },
        {%- when None %}
        {%- endmatch %}
        {%- endfor %}
        uniffi_free: (uniffiHandle: UniffiHandle): void => {
            // this will throw a stale handle error if the handle isn't found.
            {{ ffi_converter_name }}.drop(uniffiHandle);
        },
        uniffi_clone: (uniffiHandle: UniffiHandle): UniffiHandle => {
            return {{ ffi_converter_name }}.clone(uniffiHandle);
        }
    },
    register: () => {
        {%- call cb::native_method_handle(vtable.ffi_init_fn) %}(
            {{ trait_impl }}.vtable
        );
    },
};
{%- endmacro %}
