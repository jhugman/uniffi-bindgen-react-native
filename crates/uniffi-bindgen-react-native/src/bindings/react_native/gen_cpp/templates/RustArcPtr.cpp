namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<void *> {
  static void* fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      auto num = uniffi_jsi::Bridging<uint64_t>::fromJs(rt, value);
      return reinterpret_cast<void*>(static_cast<uintptr_t>(num));
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, void* value) {
    auto num = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(value));
    return uniffi_jsi::Bridging<uint64_t>::toJs(rt, num);
  }
};
} // namespace uniffi_jsi
