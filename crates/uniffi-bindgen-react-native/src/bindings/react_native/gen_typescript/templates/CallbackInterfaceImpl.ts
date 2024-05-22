{%- if self.include_once_check("CallbackInterfaceRuntime.ts") %}{%- include "CallbackInterfaceRuntime.ts" %}{%- endif %}
{#
{%- let trait_impl=format!("UniffiCallbackInterface{}", name) %}

// Put the implementation in a struct so we don't pollute the top-level namespace
class {{ trait_impl }} {

    // Create the VTable using a series of closures.
    // ts automatically converts these into C callback functions.
    static vtable: {{ vtable|ffi_type_name }} = new {{ vtable|ffi_type_name }}({
        {%- for (ffi_callback, meth) in vtable_methods %}
        {{ meth.name()|fn_name }}: (
            {%- for arg in ffi_callback.arguments() %}
            {{ arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name }}{% if !loop.last || ffi_callback.has_rust_call_status_arg() %},{% endif %}
            {%- endfor -%}
            {%- if ffi_callback.has_rust_call_status_arg() %}
            uniffiCallStatus: UnsafeMutablePointer<UniffiRustCallStatus>
            {%- endif %}
        ) => {
            const makeCall = {# space #}
            {%- if meth.is_async() %}async {% else %} {% endif-%}
            (): {% call ts::return_type(meth) %} => {
                // Was converter.handleMap.get(uniffiHandle);
                const uniffiObj = {{ ffi_converter_name }}.lift(uniffiHandle);
                if (uniffiObj === undefined) {
                    throw new UniffiInternalError.UnexpectedStaleHandle()
                }
                return {% if meth.is_async() %}await {% endif %}uniffiObj.{{ meth.name()|fn_name }}(
                    {%- for arg in meth.arguments() %}
                    {{ arg|lift_fn }}({{ arg.name()|var_name }}){% if !loop.last %},{% endif %}
                    {%- endfor %}
                )
            }
            {%- if !meth.is_async() %}

            {% match meth.return_type() %}
            {%- when Some(t) %}
            const writeReturn = (obj: any) => { uniffiOutReturn.pointee = {{ t|lower_fn }}(obj) };
            {%- when None %}
            const writeReturn = (obj: any) => {};
            {%- endmatch %}

            {%- match meth.throws_type() %}
            {%- when None %}
            uniffiTraitInterfaceCall(
                /*callStatus:*/ uniffiCallStatus,
                /*makeCall:*/ makeCall,
                /*writeReturn:*/ writeReturn
            )
            {%- when Some(error_type) %}
            uniffiTraitInterfaceCallWithError(
                /*callStatus:*/ uniffiCallStatus,
                /*makeCall:*/ makeCall,
                /*writeReturn:*/ writeReturn,
                /*lowerError:*/ {{ error_type|lower_fn }}
            )
            {%- endmatch %}
            {%- else %}

            let uniffiHandleSuccess = { (returnValue: {% call ts::return_type(meth) %}) in
                uniffiFutureCallback(
                    uniffiCallbackData,
                    {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }}(
                        {%- match meth.return_type() %}
                        {%- when Some(return_type) %}
                        /*returnValue:*/ {{ return_type|lower_fn }}(returnValue),
                        {%- when None %}
                        {%- endmatch %}
                        /*callStatus:*/ new UniffiRustCallStatus()
                    )
                )
            }
            let uniffiHandleError = { (statusCode, errorBuf) in
                uniffiFutureCallback(
                    uniffiCallbackData,
                    {{ meth.foreign_future_ffi_result_struct().name()|ffi_struct_name }}(
                        {%- match meth.return_type().map(FfiType::from) %}
                        {%- when Some(return_type) %}
                        /*returnValue:*/ {{ return_type|ffi_default_value }},
                        {%- when None %}
                        {%- endmatch %}
                        /*callStatus:*/ { statusCode, errorBuf: errorBuf }
                    )
                )
            }

            {%- match meth.throws_type() %}
            {%- when None %}
            let uniffiForeignFuture = uniffiTraitInterfaceCallAsync(
                /*makeCall:*/ makeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError
            )
            {%- when Some(error_type) %}
            let uniffiForeignFuture = uniffiTraitInterfaceCallAsyncWithError(
                /*makeCall:*/ makeCall,
                /*handleSuccess:*/ uniffiHandleSuccess,
                /*handleError:*/ uniffiHandleError,
                /*lowerError:*/ {{ error_type|lower_fn }}
            )
            {%- endmatch %}
            uniffiOutReturn.pointee = uniffiForeignFuture
            {%- endif %}
        },
        {%- endfor %}
        uniffiFree: (uniffiHandle: UniffiHandle): void => {
            let result = {{ ffi_converter_name }}.handleMap.remove(uniffiHandle)
            if (result === undefined) {
                print("Uniffi callback interface {{ name }}: handle missing in uniffiFree")
            }
        }
        }
    )
}

// TODO: callback interface initialization
function {{ callback_init }}() {
    /*{{ ffi_init_callback.name() }}(&{{ trait_impl }}.vtable)*/
}
#}
