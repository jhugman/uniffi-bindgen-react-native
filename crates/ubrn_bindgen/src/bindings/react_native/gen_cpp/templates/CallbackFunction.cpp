{%- let name = callback.name()|ffi_callback_name %}

// Callback function: {{ ns }}::{{ name }}
//
// We have the following constraints:
// - we need to pass a function pointer to Rust.
// - we need a jsi::Runtime and jsi::Function to call into JS.
// - function pointers can't store state, so we can't use a lamda.
//
// For this, we store a lambda as a global, as `rsLambda`. The `callback` function calls
// the lambda, which itself calls the `body` which then calls into JS.
//
// We then give the `callback` function pointer to Rust which will call the lambda sometime in the
// future.
namespace {{ ns }} {
    using namespace facebook;

    // We need to store a lambda in a global so we can call it from
    // a function pointer. The function pointer is passed to Rust.
    static std::function<void(
        {%- for arg in callback.arguments() %}
        {%-   let arg_t = arg.type_().borrow()|ffi_type_name %}
        {{- arg_t }}
        {%- if !loop.last %}, {% endif %}
        {%- endfor %}
        {%- if callback.has_rust_call_status_arg() -%}
        , RustCallStatus*
        {%- endif -%})> rsLambda = nullptr;

    // This is the main body of the callback. It's called from the lambda,
    // which itself is called from the callback function which is passed to Rust.
    static void body(jsi::Runtime &rt,
                     std::shared_ptr<uniffi_runtime::UniffiCallInvoker> callInvoker,
                     std::shared_ptr<jsi::Value> callbackValue
            {%- for arg in callback.arguments() %}
            {%-   let arg_t = arg.type_().borrow()|ffi_type_name %}
            {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
            , {{- arg_t }} {{ arg_nm_rs }}
            {%- endfor %}
            {%- if callback.has_rust_call_status_arg() -%}
            , RustCallStatus* uniffi_call_status
            {%- endif -%}) {

        // Convert the arguments from Rust, into jsi::Values.
        // We'll use the Bridging class to do this…
        {%- for arg in callback.arguments() %}
        {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
        {%-   let arg_t = arg.type_().borrow()|ffi_type_name_from_js %}
        {%-   let arg_nm = arg.name()|var_name|fmt("js_{}") %}
        {%-   if arg.is_return() %}
        // … but we need to take extra care for the return value.
        // In the typescript we use a dummy object we called a ReferenceHolder.
        auto {{ arg_nm }} = {{ ci.cpp_namespace_includes() }}::Bridging<ReferenceHolder<{{ arg_t }}>>::jsNew(rt);
        {%-   else %}
        auto {{ arg_nm }} = {{ arg.type_().borrow()|bridging_class(ci) }}::toJs(rt, callInvoker, {{ arg_nm_rs }});
        {%-   endif %}
        {%- endfor %}

        {% if callback.has_rust_call_status_arg() -%}
        // The RustCallStatus is another very simple JS object which will
        // report errors back to Rust.
        auto uniffiCallStatus = {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::jsSuccess(rt);
        {%- endif %}

        // Now we are ready to call the callback.
        // We should be using callInvoker at this point, but for now
        // we think that there are no threading issues to worry about.
        try {
            // Getting the callback function
            auto cb = callbackValue->asObject(rt).asFunction(rt);
            cb.call(rt
            {%- for arg in callback.arguments() %}
            {%-   let arg_nm = arg.name()|var_name|fmt("js_{}") -%}
                , {{ arg_nm }}
            {%- endfor %}
            {%- if callback.has_rust_call_status_arg() -%}
                , uniffiCallStatus
            {%- endif %}
            );

            {% if callback.has_rust_call_status_arg() -%}
            // Now copy the result back from JS into the RustCallStatus object.
            {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::copyFromJs(rt, callInvoker, uniffiCallStatus, uniffi_call_status);

            if (uniffi_call_status->code != UNIFFI_CALL_STATUS_OK) {
                // The JS callback finished abnormally, so we cannot retrieve the return value.
                return;
            }
            {%- endif %}

            {% match callback.arg_return_type() -%}
            {%- when Some with (arg_t) %}
            // Finally, we need to copy the return value back into the Rust pointer.
            *rs_uniffiOutReturn =
                {{ arg_t|bridging_namespace(ci) }}::Bridging<ReferenceHolder<{{ arg_t|ffi_type_name_from_js }}>>::fromJs(
                    rt, callInvoker, js_uniffiOutReturn
                );
            {%- else %}
            {%- endmatch %}
        } catch (const jsi::JSError &error) {
            std::cout << "Error in callback {{ name }}: "
                    << error.what() << std::endl;
            {%- if callback.has_rust_call_status_arg() %}
            {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::copyFromJs(
                rt, callInvoker, uniffiCallStatus, uniffi_call_status);
            {%- endif %}
        }
    }

    static void callback(
            {%- for arg in callback.arguments() %}
            {%-   let arg_t = arg.type_().borrow()|ffi_type_name %}
            {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
            {{- arg_t }} {{ arg_nm_rs }}
            {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            {%- if callback.has_rust_call_status_arg() -%}
            , RustCallStatus* uniffi_call_status
            {%- endif -%}) {
        // If the runtime has shutdown, then there is no point in trying to
        // call into Javascript. BUT how do we tell if the runtime has shutdown?
        //
        // Answer: the module destructor calls into callback `cleanup` method,
        // which nulls out the rsLamda.
        //
        // If rsLamda is null, then there is no runtime to call into.
        if (rsLambda == nullptr) {
            // This only occurs when destructors are calling into Rust free/drop,
            // which causes the JS callback to be dropped.
            return;
        }

        // The runtime, the actual callback jsi::funtion, and the callInvoker
        // are all in the lambda.
        rsLambda(
            {%- for arg in callback.arguments() %}
            {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
            {{ arg_nm_rs }}
            {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            {%- if callback.has_rust_call_status_arg() -%}
            , uniffi_call_status
            {%- endif -%}
        );
    }

    static {{ name }}
    makeCallbackFunction( // {{ ns }}
                    jsi::Runtime &rt,
                     std::shared_ptr<uniffi_runtime::UniffiCallInvoker> callInvoker,
                     const jsi::Value &value) {
        auto callbackFunction = value.asObject(rt).asFunction(rt);
        auto callbackValue = std::make_shared<jsi::Value>(rt, callbackFunction);
        rsLambda = [&rt, callInvoker, callbackValue](
            {%- for arg in callback.arguments() %}
            {%-   let arg_t = arg.type_().borrow()|ffi_type_name %}
            {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
            {{- arg_t }} {{ arg_nm_rs }}
            {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            {%- if callback.has_rust_call_status_arg() -%}
            , RustCallStatus* uniffi_call_status
            {%- endif -%}
            ) {
                // We immediately make a lambda which will do the work of transforming the
                // arguments into JSI values and calling the callback.
                uniffi_runtime::UniffiCallFunc jsLambda = [
                    callInvoker,
                    callbackValue
                    {%- for arg in callback.arguments() %}
                    {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
                    , {{ arg_nm_rs }}
                    {%- endfor %}
                    {%- if callback.has_rust_call_status_arg() -%}
                    , uniffi_call_status
                    {%- endif -%}
                ](jsi::Runtime &rt) mutable {
                    body(rt, callInvoker, callbackValue
                        {%- for arg in callback.arguments() %}
                        {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
                        , {{ arg_nm_rs }}
                        {%- endfor %}
                        {%- if callback.has_rust_call_status_arg() -%}
                        , uniffi_call_status
                        {%- endif -%}
                    );
                };
                // We'll then call that lambda from the callInvoker which will
                // look after calling it on the correct thread.
                {% if callback.is_blocking() -%}
                callInvoker->invokeBlocking(rt, jsLambda);
                {%- else %}
                callInvoker->invokeNonBlocking(rt, jsLambda);
                {%- endif %}
        };
        return callback;
    }

    // This method is called from the destructor of {{ module_name }}, which only happens
    // when the jsi::Runtime is being destroyed.
    static void cleanup() {
        // The lambda holds a reference to the the Runtime, so when this is nulled out,
        // then the pointer will no longer be left dangling.
        rsLambda = nullptr;
    }
} // namespace {{ ns }}
