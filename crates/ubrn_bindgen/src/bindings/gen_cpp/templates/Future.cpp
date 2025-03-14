{%- let ns = ci.cpp_namespace() %}
{%- let cb_type = FfiType::Callback("RustFutureContinuationCallback".to_string()) %}
namespace {{ ns }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<UniffiRustFutureContinuationCallback> {
  static UniffiRustFutureContinuationCallback fromJs(
    jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &value
  ) {
    try {
      return {{ cb_type.borrow()|cpp_namespace(ci) }}::makeCallbackFunction(
        rt,
        callInvoker,
        value
      );
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace {{ ns }}
