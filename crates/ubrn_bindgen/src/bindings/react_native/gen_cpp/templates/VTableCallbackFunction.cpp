{%- let name = callback.name()|ffi_callback_name %}
{%- let ns = name|lower|fmt("uniffi_jsi::{}") %}

// Callback function: {{ name }}
//
// We have the following constraints:
// - we need to pass a function pointer to Rust.
// - we need a jsi::Runtime and jsi::Function to call into JS.
// - function pointers can't store state, so we can't use a lamda.
//
// For this, we store a lambda as a global, as `lambda`. The `callback` function calls
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
        {%- endif -%})> lambda = nullptr;

    // This is the main body of the callback. It's called from the lambda,
    // which itself is called from the callback function which is passed to Rust.
    static void body(jsi::Runtime &rt,
                     std::shared_ptr<react::CallInvoker> callInvoker,
                     jsi::Function &cb
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
        auto {{ arg_nm }} = uniffi_jsi::Bridging<ReferenceHolder<{{ arg_t }}>>::jsNew(rt);
        {%-   else %}
        auto {{ arg_nm }} = uniffi_jsi::Bridging<{{ arg_t }}>::toJs(rt, {{ arg_nm_rs }});
        {%-   endif %}
        {%- endfor %}

        {% if callback.has_rust_call_status_arg() -%}
        // The RustCallStatus is another very simple JS object which will
        // report errors back to Rust.
        auto uniffiCallStatus = uniffi_jsi::Bridging<RustCallStatus>::jsSuccess(rt);
        {%- endif %}

        // Now we are ready to call the callback.
        // We should be using callInvoker at this point, but for now
        // we think that there are no threading issues to worry about.
        try {
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
            uniffi_jsi::Bridging<RustCallStatus>::copyFromJs(rt, uniffiCallStatus, *uniffi_call_status);
            {%- endif %}

            {% match callback.arg_return_type() -%}
            {%- when Some with (arg_t) %}
            // Finally, we need to copy the return value back into the Rust pointer.
            *rs_uniffiOutReturn = uniffi_jsi::Bridging<ReferenceHolder<{{ arg_t|ffi_type_name_from_js }}>>::fromJs(rt, js_uniffiOutReturn);
            {%- else %}
            {%- endmatch %}
        } catch (const jsi::JSError &error) {
            std::cout << "Error in callback {{ name }}: "
                    << error.what() << std::endl;
            {%- if callback.has_rust_call_status_arg() %}
            uniffi_jsi::Bridging<RustCallStatus>::copyFromJs(
                rt, uniffiCallStatus, *uniffi_call_status);
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
        // The runtime, the actual callback jsi::funtion, and the callInvoker
        // are all in the lambda.
        lambda(
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
    makeCallbackFunction(jsi::Runtime &rt,
                     std::shared_ptr<react::CallInvoker> callInvoker,
                     jsi::Function &cb) {
        lambda = [&rt, callInvoker, &cb](
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
                body(rt, callInvoker, cb
                    {%- for arg in callback.arguments() %}
                    {%-   let arg_nm_rs = arg.name()|var_name|fmt("rs_{}") %}
                    , {{ arg_nm_rs }}
                    {%- endfor %}
                    {%- if callback.has_rust_call_status_arg() -%}
                    , uniffi_call_status
                    {%- endif -%}
                );
        };
        return callback;
    }
} // namespace {{ ns }}
