{%- let ns = ci.cpp_namespace() %}
{%- let cb_type = FfiType::Callback("RustFutureContinuationCallback".to_string()) %}
{%- let future_ns = ci.namespace() %}
{%- let future_ns_cap = future_ns|capitalize %}
namespace {{ ns }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

// Wrapper struct to make the Bridging<T> specialization unique per module.
// Without this, when multiple crates define async functions, they would all try
// to specialize Bridging<UniffiRustFutureContinuationCallback>, causing ODR violations.
// By wrapping in a module-specific type, each crate gets its own Bridging specialization.
struct UniffiRustFutureContinuationCallback{{ future_ns_cap }}Wrapper {
    UniffiRustFutureContinuationCallback callback;
    explicit UniffiRustFutureContinuationCallback{{ future_ns_cap }}Wrapper(UniffiRustFutureContinuationCallback cb) : callback(cb) {}
    operator UniffiRustFutureContinuationCallback() const { return callback; }
};

template <> struct Bridging<UniffiRustFutureContinuationCallback{{ future_ns_cap }}Wrapper> {
  static UniffiRustFutureContinuationCallback{{ future_ns_cap }}Wrapper fromJs(
    jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &value
  ) {
    try {
      return UniffiRustFutureContinuationCallback{{ future_ns_cap }}Wrapper(
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
