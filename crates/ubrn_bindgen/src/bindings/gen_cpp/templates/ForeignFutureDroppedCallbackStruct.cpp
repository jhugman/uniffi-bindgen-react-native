{%- let cb_name = callback.name()|ffi_callback_name %}
namespace {{ ci.cpp_namespace() }} {
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<{{ cb_name }}> {
  static jsi::Value toJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker> callInvoker, {{ cb_name}} rsCallback) {
    {%- let cb_id = callback.name()|fmt("--{}") %}
    return jsi::Function::createFromHostFunction(
        rt,
        jsi::PropNameID::forAscii(rt, "{{ cb_id }}"),
        {{ callback.arguments().len() }},
        [rsCallback, callInvoker](
            jsi::Runtime &rt,
            const jsi::Value &thisValue,
            const jsi::Value *arguments,
            size_t count) -> jsi::Value
        {
            return intoRust(rt, callInvoker, thisValue, arguments, count, rsCallback);
        }
    );
  }

  static jsi::Value intoRust(
      jsi::Runtime &rt,
      std::shared_ptr<CallInvoker> callInvoker,
      const jsi::Value &thisValue,
      const jsi::Value *args,
      size_t count,
      {{ cb_name}} func) {
    // Convert the arguments into the Rust, with Bridging<T>::fromJs,
    // then call the rs_callback with those arguments.
    {%- call cpp::cpp_fn_rust_caller_body_with_func_name(callback, "func") %}
  }
};
} // namespace {{ ci.cpp_namespace() }}
