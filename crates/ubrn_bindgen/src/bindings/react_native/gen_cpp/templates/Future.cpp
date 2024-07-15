namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<UniffiRustFutureContinuationCallback> {
  static UniffiRustFutureContinuationCallback fromJs(
    jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &value
  ) {
    try {
      auto fn = value.asObject(rt).asFunction(rt);
      static auto callback = uniffi_jsi::uniffirustfuturecontinuationcallback::makeCallbackFunction(
        rt,
        callInvoker,
        fn
      );
      return callback;
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace uniffi_jsi
