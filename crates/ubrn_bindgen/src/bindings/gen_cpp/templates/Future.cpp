{%- let ns = ci.cpp_namespace() %}
{%- let cb_type = FfiType::Callback("RustFutureContinuationCallback".to_string()) %}
{%- let future_ns = ci.namespace() %}
namespace {{ ns }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

// Wrapper to make this callback unique per module  
struct UniffiRustFutureContinuationCallback{{ future_ns|capitalize }}Wrapper {
    UniffiRustFutureContinuationCallback callback;
    explicit UniffiRustFutureContinuationCallback{{ future_ns|capitalize }}Wrapper(UniffiRustFutureContinuationCallback cb) : callback(cb) {}
    operator UniffiRustFutureContinuationCallback() const { return callback; }
};

template <> struct Bridging<UniffiRustFutureContinuationCallback{{ future_ns|capitalize }}Wrapper> {
  static UniffiRustFutureContinuationCallback{{ future_ns|capitalize }}Wrapper fromJs(
    jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &value
  ) {
    try {
      return UniffiRustFutureContinuationCallback{{ future_ns|capitalize }}Wrapper(
        {{ cb_type.borrow()|cpp_namespace(ci) }}::makeCallbackFunction(
          rt,
          callInvoker,
          value
        )
      );
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace {{ ns }}
