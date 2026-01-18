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

    {% let ret_type %}
    {% match callback.return_type() %}
      {% when Some with (val) %}
        {% let ret_type = val|ffi_type_name() %}
      {% when None %}
        {% let ret_type = "void".into() %}
    {% endmatch %}

    // We need to store a lambda in a global so we can call it from
    // a function pointer. The function pointer is passed to Rust.
    static std::function<{{ ret_type }}(
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
    static {{ ret_type }} body(jsi::Runtime &rt,
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
        {%- for arg in callback.arguments_no_return() %}
        {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
        {%-   let arg_t = arg.type_().borrow()|ffi_type_name_from_js %}
        {%-   let arg_nm = arg.name()|var_name|fmt("js_{}") %}
        auto {{ arg_nm }} = {{ arg.type_().borrow()|bridging_class(ci) }}::toJs(rt, callInvoker, {{ arg_nm_rs }});
        {%- endfor %}

        // Now we are ready to call the callback.
        // We are already on the JS thread, because this `body` function was
        // invoked from the CallInvoker.
        try {
            // Getting the callback function
            auto cb = callbackValue->asObject(rt).asFunction(rt);
            auto uniffiResult = cb.call(rt
            {%- for arg in callback.arguments_no_return() %}
            {%-   let arg_nm = arg.name()|var_name|fmt("js_{}") -%}
                , {{ arg_nm }}
            {%- endfor %}
            );

            {% if callback.has_rust_call_status_arg() -%}
            // Now copy the result back from JS into the RustCallStatus object.
            {{ ci.cpp_namespace() }}::Bridging<RustCallStatus>::copyFromJs(rt, callInvoker, uniffiResult, uniffi_call_status);

            if (uniffi_call_status->code != UNIFFI_CALL_STATUS_OK) {
                // The JS callback finished abnormally, so we cannot retrieve the return value.
                return;
            }
            {%- endif %}

            {% match callback.arg_return_type() -%}
            {%- when Some with (arg_t) %}
            // return type is {{ arg_t|fmt("{:?}") }}
            {%- let is_async = arg_t.is_foreign_future() %}
            {%- let arg_t_label = arg_t|ffi_type_name_from_js %}
            // Finally, we need to copy the return value back into the Rust pointer.
            *rs_uniffiOutReturn =
                {{ arg_t|bridging_namespace(ci) }}::Bridging<
                {%- if is_async %}
                    {{ arg_t_label }}
                {%- else %}
                    ReferenceHolder<{{ arg_t_label }}>
                {%- endif %}
                >::fromJs(
                    rt, callInvoker, uniffiResult
                );
            {%- else %}
            {%- endmatch %}
            {% if ret_type != "void" %}
            return uniffi_jsi::Bridging<{{ret_type}}>::fromJs(rt, callInvoker, uniffiResult);
            {% endif %}
        } catch (const jsi::JSError &error) {
            std::cout << "Error in callback {{ name }}: "
                    << error.what() << std::endl;
            throw error;
        }
    }

    static {{ret_type}} callback(
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
            {% if ret_type == "uint64_t" -%}
                return 0ULL;
            {%- else -%}
                return;
            {%- endif %}

        }

        // The runtime, the actual callback jsi::funtion, and the callInvoker
        // are all in the lambda.
        {% if ret_type != "void" -%}return {% endif %}rsLambda(
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
        if (rsLambda != nullptr) {
            // `makeCallbackFunction` is called in two circumstances:
            //
            // 1. at startup, when initializing callback interface vtables.
            // 2. when polling futures. This happens at least once per future that is
            //    exposed to Javascript. We know that this is always the same function,
            //    `uniffiFutureContinuationCallback` in `async-rust-calls.ts`.
            //
            // We can therefore return the callback function without making anything
            // new if we've been initialized already.
            return callback;
        }
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
                {% if ret_type == "uint64_t" -%}
                auto result = std::make_shared<uint64_t>(0);
                {%- endif %}

                // We immediately make a lambda which will do the work of transforming the
                // arguments into JSI values and calling the callback.
                uniffi_runtime::UniffiCallFunc jsLambda = [
                    {% if ret_type != "void" %}result,{% endif %}
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
                    {% if ret_type != "void" %}*result = {% endif %}body(rt, callInvoker, callbackValue
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
                {% else if ret_type != "void" -%}
                callInvoker->invokeBlocking(rt, jsLambda);
                return *result;
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
