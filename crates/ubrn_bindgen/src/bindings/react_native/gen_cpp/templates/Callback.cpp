
struct Callback {};

namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<Callback> {
  static Callback fromJs(jsi::Runtime &rt, const jsi::Value &value, std::shared_ptr<CallInvoker>) {
    try {
      throw jsi::JSError(rt, "Unimplemented");
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, Callback value) {
    throw jsi::JSError(rt, "Unimplemented");
  }
};

} // namespace uniffi_jsi
