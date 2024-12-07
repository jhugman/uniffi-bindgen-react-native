
namespace {{ ci.cpp_namespace() }} {
template <typename T> struct Bridging;

using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <typename T> struct Bridging<ReferenceHolder<T>> {
  static jsi::Value jsNew(jsi::Runtime &rt) {
    auto holder = jsi::Object(rt);
    return holder;
  }
  static T fromJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker> callInvoker,
                  const jsi::Value &value) {
    auto obj = value.asObject(rt);
    if (obj.hasProperty(rt, "pointee")) {
      auto pointee = obj.getProperty(rt, "pointee");
      return {{ ci.cpp_namespace() }}::Bridging<T>::fromJs(rt, callInvoker, pointee);
    }
    throw jsi::JSError(
      rt,
      "Expected ReferenceHolder to have a pointee property. This is likely a bug in uniffi-bindgen-react-native"
    );
  }
};
} // namespace {{ ci.cpp_namespace() }}
